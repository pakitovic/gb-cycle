use gb_core::{
    CartridgeDiagnostic, CartridgeLoadError, CartridgePersistentStateError, CartridgeSlot,
    JoypadButton, Machine, MachineConfig, MachineSaveState, MachineSaveStateRestoreError,
    PersistentCartState, TraceBuffer, TraceSummaryBuffer,
};

pub(crate) enum CliMachine {
    Buffered(Machine<TraceBuffer>),
    Summary(Machine<TraceSummaryBuffer>),
}

impl CliMachine {
    pub(crate) fn new(config: MachineConfig, capture_trace: bool) -> Self {
        if capture_trace {
            Self::Buffered(Machine::new(config))
        } else {
            Self::Summary(Machine::new_summary(config))
        }
    }

    pub(crate) fn load_cartridge(
        &mut self,
        rom_bytes: Vec<u8>,
    ) -> Result<Vec<CartridgeDiagnostic>, CartridgeLoadError> {
        match self {
            Self::Buffered(machine) => machine.load_cartridge(rom_bytes),
            Self::Summary(machine) => machine.load_cartridge(rom_bytes),
        }
    }

    pub(crate) fn step_t_cycle(&mut self) {
        match self {
            Self::Buffered(machine) => {
                machine.step_t_cycle();
            }
            Self::Summary(machine) => {
                machine.step_t_cycle();
            }
        }
    }

    pub(crate) fn set_joypad_button_pressed(&mut self, button: JoypadButton, pressed: bool) {
        match self {
            Self::Buffered(machine) => {
                machine.set_joypad_button_pressed(button, pressed);
            }
            Self::Summary(machine) => {
                machine.set_joypad_button_pressed(button, pressed);
            }
        }
    }

    pub(crate) fn take_serial_output_bytes(&mut self) -> Vec<u8> {
        match self {
            Self::Buffered(machine) => machine.take_serial_output_bytes(),
            Self::Summary(machine) => machine.take_serial_output_bytes(),
        }
    }

    pub(crate) fn at_frame_origin(&self) -> bool {
        match self {
            Self::Buffered(machine) => machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0,
            Self::Summary(machine) => machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0,
        }
    }

    pub(crate) fn is_boot_rom_mapped(&self) -> bool {
        match self {
            Self::Buffered(machine) => machine.boot().is_boot_rom_mapped(),
            Self::Summary(machine) => machine.boot().is_boot_rom_mapped(),
        }
    }

    pub(crate) fn framebuffer(&self) -> &[u8] {
        match self {
            Self::Buffered(machine) => machine.ppu().framebuffer(),
            Self::Summary(machine) => machine.ppu().framebuffer(),
        }
    }

    pub(crate) fn cgb_framebuffer_rgb555(&self) -> Option<&[u16]> {
        match self {
            Self::Buffered(machine) => machine.ppu().cgb_framebuffer_rgb555(),
            Self::Summary(machine) => machine.ppu().cgb_framebuffer_rgb555(),
        }
    }

    pub(crate) fn sgb_framebuffer_rgb555(&self) -> Option<Vec<u16>> {
        match self {
            Self::Buffered(machine) => machine.sgb_framebuffer_rgb555(),
            Self::Summary(machine) => machine.sgb_framebuffer_rgb555(),
        }
    }

    pub(crate) fn sgb_lcd_framebuffer_rgb555(&self) -> Option<Vec<u16>> {
        match self {
            Self::Buffered(machine) => machine.sgb_lcd_framebuffer_rgb555(),
            Self::Summary(machine) => machine.sgb_lcd_framebuffer_rgb555(),
        }
    }

    pub(crate) fn cartridge(&self) -> &CartridgeSlot {
        match self {
            Self::Buffered(machine) => machine.cartridge(),
            Self::Summary(machine) => machine.cartridge(),
        }
    }

    pub(crate) fn restore_cartridge_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        match self {
            Self::Buffered(machine) => machine.restore_cartridge_persistent_state(state),
            Self::Summary(machine) => machine.restore_cartridge_persistent_state(state),
        }
    }

    pub(crate) fn capture_save_state(&self) -> MachineSaveState {
        match self {
            Self::Buffered(machine) => machine.capture_save_state(),
            Self::Summary(machine) => machine.capture_save_state(),
        }
    }

    pub(crate) fn restore_save_state(
        &mut self,
        state: &MachineSaveState,
    ) -> Result<(), MachineSaveStateRestoreError> {
        match self {
            Self::Buffered(machine) => machine.restore_save_state(state),
            Self::Summary(machine) => machine.restore_save_state(state),
        }
    }

    pub(crate) fn trace_text(&self) -> Option<String> {
        match self {
            Self::Buffered(machine) => Some(machine.tracer().sink().render_text()),
            Self::Summary(_) => None,
        }
    }
}
