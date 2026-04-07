use super::*;
use crate::bus::{Bus, BusArbitrationState, BusIoReadView, BusIoWriteView, BusRequester};
use crate::cartridge::CartridgeSlot;
use crate::interrupts::InterruptController;
use crate::joypad::Joypad;
use crate::model::CompatibilityPolicy;

mod cb;
mod control_flow;
mod internal;
mod interrupts;
mod invariants;
mod timing;

fn build_test_rom(program: &[u8]) -> Vec<u8> {
    let mut rom = vec![0xFF; 32 * 1024];
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0100 + offset] = byte;
    }
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
}

fn build_test_cartridge(program: &[u8]) -> CartridgeSlot {
    let compatibility = CompatibilityPolicy::default();
    let report = CartridgeSlot::load(build_test_rom(program), &compatibility)
        .expect("test cartridge should load as NoMBC");
    let (cartridge, _) = report.into_parts();
    cartridge
}

fn tick_cpu(cpu: &mut CpuCore, bus: &mut Bus, cartridge: &mut CartridgeSlot) {
    let arbitration_state = BusArbitrationState::default();

    cpu.tick_t_cycle(|operation| match operation {
        CpuBusOperation::Read { address } => Some(bus.read_with_context(
            address,
            BusRequester::Cpu,
            &arbitration_state,
            Some(&*cartridge),
            BusIoReadView::default(),
        )),
        CpuBusOperation::Write { address, value } => {
            bus.write_with_context(
                address,
                value,
                BusRequester::Cpu,
                &arbitration_state,
                Some(cartridge),
                BusIoWriteView::default(),
            );
            None
        }
        CpuBusOperation::PendingInterruptMask => Some(0),
        CpuBusOperation::InterruptEnableMask => Some(0),
        CpuBusOperation::StopWakeLineAsserted => Some(0),
        CpuBusOperation::AcknowledgeInterrupt { .. } => None,
        CpuBusOperation::RequestInterrupt { .. } => None,
    });
}

fn tick_cpu_with_interrupts(
    cpu: &mut CpuCore,
    bus: &mut Bus,
    cartridge: &mut CartridgeSlot,
    interrupts: &mut InterruptController,
) {
    let arbitration_state = BusArbitrationState::default();
    cpu.tick_t_cycle(|operation| match operation {
        CpuBusOperation::Read { address } => Some(bus.read_with_context(
            address,
            BusRequester::Cpu,
            &arbitration_state,
            Some(&*cartridge),
            BusIoReadView {
                interrupts: Some(&*interrupts),
                ..BusIoReadView::default()
            },
        )),
        CpuBusOperation::Write { address, value } => {
            bus.write_with_context(
                address,
                value,
                BusRequester::Cpu,
                &arbitration_state,
                Some(cartridge),
                BusIoWriteView {
                    interrupts: Some(interrupts),
                    ..BusIoWriteView::default()
                },
            );
            None
        }
        CpuBusOperation::PendingInterruptMask => Some(interrupts.pending_mask()),
        CpuBusOperation::InterruptEnableMask => Some(interrupts.read_ie()),
        CpuBusOperation::StopWakeLineAsserted => Some(0),
        CpuBusOperation::AcknowledgeInterrupt { source } => {
            interrupts.clear(source);
            None
        }
        CpuBusOperation::RequestInterrupt { source } => {
            interrupts.request(source);
            None
        }
    });
}

fn tick_cpu_n(cpu: &mut CpuCore, bus: &mut Bus, cartridge: &mut CartridgeSlot, steps: usize) {
    for _ in 0..steps {
        tick_cpu(cpu, bus, cartridge);
    }
}

fn tick_cpu_n_with_interrupts(
    cpu: &mut CpuCore,
    bus: &mut Bus,
    cartridge: &mut CartridgeSlot,
    interrupts: &mut InterruptController,
    steps: usize,
) {
    for _ in 0..steps {
        tick_cpu_with_interrupts(cpu, bus, cartridge, interrupts);
    }
}

fn crc32_iso_hdlc(mut crc: u32, byte: u8) -> u32 {
    crc ^= u32::from(byte);
    for _ in 0..8 {
        if crc & 1 != 0 {
            crc = (crc >> 1) ^ 0xEDB8_8320;
        } else {
            crc >>= 1;
        }
    }
    crc
}
