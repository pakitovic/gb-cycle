use std::fs;
use std::path::PathBuf;

use gb_core::debugger::TraceSink;
use gb_core::{BootRomAssets, Dmg07Participant, LinkedMachines, Machine, MachineConfig};

use super::{
    LinkedMachineBuild, LinkedSessionExecutionError, LinkedSessionParticipant, LinkedSessionRunner,
    LoadedParticipantMachine, RunnerLinkedMachines,
};
use crate::{
    ExternalStimulusAction, LinkedSessionCase, boot_rom_kind_for_console_model,
    compatibility_for_execution_mode, discover_boot_rom_store_root, enforce_boot_rom_verification,
};

impl LinkedSessionRunner {
    pub(super) fn build_buffered_linked_machines(
        &self,
        session: &LinkedSessionCase,
    ) -> Result<LinkedMachineBuild, LinkedSessionExecutionError> {
        self.build_linked_machines(session, Machine::new, RunnerLinkedMachines::Buffered)
    }

    pub(super) fn build_summary_linked_machines(
        &self,
        session: &LinkedSessionCase,
    ) -> Result<LinkedMachineBuild, LinkedSessionExecutionError> {
        self.build_linked_machines(session, Machine::new_summary, RunnerLinkedMachines::Summary)
    }

    fn build_linked_machines<S, F, G>(
        &self,
        session: &LinkedSessionCase,
        new_machine: F,
        wrap_linked: G,
    ) -> Result<LinkedMachineBuild, LinkedSessionExecutionError>
    where
        S: TraceSink,
        F: Fn(MachineConfig) -> Machine<S>,
        G: FnOnce(LinkedMachines<S>) -> RunnerLinkedMachines,
    {
        let mut machines = Vec::with_capacity(session.participants.len());
        let mut diagnostics = Vec::with_capacity(session.participants.len());
        let mut resolved_rom_paths = Vec::with_capacity(session.participants.len());

        for participant in &session.participants {
            let (machine, participant_diagnostics, resolved_rom_path) =
                self.load_participant_machine(participant, &new_machine)?;
            machines.push(machine);
            diagnostics.push(participant_diagnostics);
            resolved_rom_paths.push(resolved_rom_path);
        }

        let mut linked = LinkedMachines::new(machines)
            .map_err(|source| LinkedSessionExecutionError::LinkedMachines { source })?;
        self.attach_session_topology(session, &mut linked)?;

        Ok((wrap_linked(linked), diagnostics, resolved_rom_paths))
    }

    fn load_participant_machine<S, F>(
        &self,
        participant: &LinkedSessionParticipant,
        new_machine: F,
    ) -> Result<LoadedParticipantMachine<S>, LinkedSessionExecutionError>
    where
        S: TraceSink,
        F: Fn(MachineConfig) -> Machine<S>,
    {
        let resolved_rom_path = self.resolve_participant_rom_path(participant)?;
        let rom_bytes = fs::read(&resolved_rom_path).map_err(|source| {
            LinkedSessionExecutionError::FileOperation {
                path: resolved_rom_path.clone(),
                operation: "read ROM",
                source: Box::new(source),
            }
        })?;
        let boot_rom_assets = self.load_boot_rom_assets_for_participant(participant)?;
        let config = MachineConfig::new(participant.console_model)
            .with_startup_mode(participant.startup_mode)
            .with_compatibility(compatibility_for_execution_mode(participant.execution_mode))
            .with_boot_rom_assets(boot_rom_assets);
        let mut machine = new_machine(config);
        let participant_diagnostics = machine.load_cartridge(rom_bytes).map_err(|source| {
            LinkedSessionExecutionError::CartridgeLoad {
                participant_id: participant.id.clone(),
                path: resolved_rom_path.clone(),
                source: Box::new(source),
            }
        })?;

        Ok((machine, participant_diagnostics, resolved_rom_path))
    }

    fn attach_session_topology<S: TraceSink>(
        &self,
        session: &LinkedSessionCase,
        linked: &mut LinkedMachines<S>,
    ) -> Result<(), LinkedSessionExecutionError> {
        match session.topology {
            crate::LinkedSessionTopology::Dmg04 => linked
                .attach_dmg04_cable()
                .map_err(|source| LinkedSessionExecutionError::LinkedMachines { source }),
            crate::LinkedSessionTopology::Dmg07 => {
                let participants = session
                    .participants
                    .iter()
                    .enumerate()
                    .map(|(machine_index, participant)| {
                        Dmg07Participant::new(
                            machine_index,
                            participant
                                .adapter_port
                                .expect("validated DMG-07 participant should have a port"),
                        )
                    })
                    .collect::<Vec<_>>();
                linked
                    .attach_dmg07_adapter(&participants)
                    .map_err(|source| LinkedSessionExecutionError::LinkedMachines { source })
            }
        }
    }

    fn resolve_participant_rom_path(
        &self,
        participant: &LinkedSessionParticipant,
    ) -> Result<PathBuf, LinkedSessionExecutionError> {
        self.runner
            .resolve_case_path(
                &participant.rom_path,
                participant.external_rom_root_key.as_deref(),
            )
            .map_err(|error| match error {
                crate::RomExecutionError::ExternalRomSourceManifest { source } => {
                    LinkedSessionExecutionError::ExternalRomSourceManifest {
                        source: Box::new(source),
                    }
                }
                crate::RomExecutionError::MissingExternalRomRoot { key, relative_path } => {
                    LinkedSessionExecutionError::MissingExternalRomRoot { key, relative_path }
                }
                other => LinkedSessionExecutionError::RomPathResolution {
                    participant_id: participant.id.clone(),
                    source: Box::new(other),
                },
            })
    }

    fn load_boot_rom_assets_for_participant(
        &self,
        participant: &LinkedSessionParticipant,
    ) -> Result<BootRomAssets, LinkedSessionExecutionError> {
        if participant.startup_mode != gb_core::StartupMode::RealBoot {
            return Ok(BootRomAssets::none());
        }

        let Some(kind) = boot_rom_kind_for_console_model(participant.console_model) else {
            return Ok(BootRomAssets::none());
        };

        let root = self
            .runner
            .boot_rom_root
            .clone()
            .or_else(|| discover_boot_rom_store_root(&self.runner.workspace_root))
            .unwrap_or_else(|| crate::boot_rom_store_root(&self.runner.workspace_root));
        let image_path = crate::boot_rom_image_path(&root, kind);
        enforce_boot_rom_verification(self.runner.boot_rom_verification_mode, &image_path, kind)
            .map_err(|issue| LinkedSessionExecutionError::BootRomVerification {
                issue: Box::new(issue),
            })?;
        if !root.is_dir() {
            return Ok(BootRomAssets::none());
        }

        BootRomAssets::from_directory(&root).map_err(|source| {
            LinkedSessionExecutionError::BootRomAssets {
                path: root,
                source: Box::new(source),
            }
        })
    }

    pub(super) fn apply_scheduled_stimuli(
        &self,
        session: &LinkedSessionCase,
        linked: &mut RunnerLinkedMachines,
        completed_frames: &[u32],
        applied_stimuli: &mut [Vec<bool>],
    ) {
        let current_t_cycle = linked.next_t_cycle();

        for (participant_index, participant) in session.participants.iter().enumerate() {
            for (stimulus_index, stimulus) in
                participant.external_stimuli.stimuli().iter().enumerate()
            {
                if applied_stimuli[participant_index][stimulus_index] {
                    continue;
                }

                let should_apply = match stimulus.when {
                    crate::StimulusTime::TCycle(t_cycle) => t_cycle == current_t_cycle,
                    crate::StimulusTime::Frame(frame) => {
                        frame == completed_frames[participant_index]
                    }
                };

                if !should_apply {
                    continue;
                }

                match stimulus.action {
                    ExternalStimulusAction::JoypadSetButton { button, pressed } => {
                        linked.set_joypad_button_pressed(participant_index, button, pressed);
                    }
                    ExternalStimulusAction::WriteMemory { address, value } => {
                        linked.write_bus(participant_index, address, value);
                    }
                }

                applied_stimuli[participant_index][stimulus_index] = true;
            }
        }
    }
}
