use crate::model::ConsoleModel;
use crate::scheduler::CycleContext;

const LCDC_ENABLE_BIT: u8 = 0x80;
const STAT_WRITABLE_ENABLE_MASK: u8 = 0x78;
const STAT_FORCED_HIGH_BIT: u8 = 0x80;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PpuAccessMode {
    #[default]
    HBlank,
    VBlank,
    OamScan,
    Drawing,
}

impl PpuAccessMode {
    pub const fn from_stat_bits(bits: u8) -> Self {
        match bits & 0x03 {
            0 => Self::HBlank,
            1 => Self::VBlank,
            2 => Self::OamScan,
            _ => Self::Drawing,
        }
    }

    pub const fn stat_bits(self) -> u8 {
        match self {
            Self::HBlank => 0,
            Self::VBlank => 1,
            Self::OamScan => 2,
            Self::Drawing => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PpuBusState {
    lcd_enabled: bool,
    mode: PpuAccessMode,
}

impl PpuBusState {
    pub const fn lcd_enabled(mode: PpuAccessMode) -> Self {
        Self {
            lcd_enabled: true,
            mode,
        }
    }

    pub const fn lcd_disabled() -> Self {
        Self {
            lcd_enabled: false,
            mode: PpuAccessMode::HBlank,
        }
    }

    pub const fn is_lcd_enabled(self) -> bool {
        self.lcd_enabled
    }

    pub const fn mode(self) -> PpuAccessMode {
        self.mode
    }
}

impl Default for PpuBusState {
    fn default() -> Self {
        Self::lcd_disabled()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpuStatus {
    RegistersReady,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DmgObjPaletteReadPolicy {
    #[default]
    ReadAsFfUntilWritten,
}

impl DmgObjPaletteReadPolicy {
    pub const fn default_read_value(self) -> u8 {
        match self {
            Self::ReadAsFfUntilWritten => 0xFF,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PpuStartupState {
    pub lcdc: u8,
    pub stat: u8,
    pub scy: u8,
    pub scx: u8,
    pub ly: u8,
    pub lyc: u8,
    pub bgp: u8,
    pub wy: u8,
    pub wx: u8,
    pub obj_palette_read_policy: DmgObjPaletteReadPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ppu {
    console_model: ConsoleModel,
    status: PpuStatus,
    lcdc: u8,
    stat_interrupt_enable: u8,
    mode: PpuAccessMode,
    scy: u8,
    scx: u8,
    ly: u8,
    lyc: u8,
    bgp: u8,
    obp0: Option<u8>,
    obp1: Option<u8>,
    wy: u8,
    wx: u8,
    obj_palette_read_policy: DmgObjPaletteReadPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PpuSnapshot {
    pub console_model: ConsoleModel,
    pub status: PpuStatus,
    pub lcdc: u8,
    pub stat_interrupt_enable: u8,
    pub mode: PpuAccessMode,
    pub scy: u8,
    pub scx: u8,
    pub ly: u8,
    pub lyc: u8,
    pub bgp: u8,
    pub obp0: Option<u8>,
    pub obp1: Option<u8>,
    pub wy: u8,
    pub wx: u8,
    pub obj_palette_read_policy: DmgObjPaletteReadPolicy,
}

impl Ppu {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: PpuStatus::RegistersReady,
            lcdc: 0,
            stat_interrupt_enable: 0,
            mode: PpuAccessMode::HBlank,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0,
            obp0: None,
            obp1: None,
            wy: 0,
            wx: 0,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> PpuStatus {
        self.status
    }

    pub fn bus_state(&self) -> PpuBusState {
        if self.is_lcd_enabled() {
            PpuBusState::lcd_enabled(self.mode)
        } else {
            PpuBusState::lcd_disabled()
        }
    }

    pub fn read_register(&self, address: u16) -> u8 {
        match address {
            0xFF40 => self.read_lcdc(),
            0xFF41 => self.read_stat(),
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.ly,
            0xFF45 => self.lyc,
            0xFF47 => self.bgp,
            0xFF48 => self
                .obp0
                .unwrap_or(self.obj_palette_read_policy.default_read_value()),
            0xFF49 => self
                .obp1
                .unwrap_or(self.obj_palette_read_policy.default_read_value()),
            0xFF4A => self.wy,
            0xFF4B => self.wx,
            _ => 0xFF,
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        match address {
            0xFF40 => self.write_lcdc(value),
            0xFF41 => self.write_stat(value),
            0xFF42 => self.scy = value,
            0xFF43 => self.scx = value,
            0xFF44 => {}
            0xFF45 => self.lyc = value,
            0xFF47 => self.bgp = value,
            0xFF48 => self.obp0 = Some(value),
            0xFF49 => self.obp1 = Some(value),
            0xFF4A => self.wy = value,
            0xFF4B => self.wx = value,
            _ => {}
        }
    }

    pub fn apply_startup_state(&mut self, startup_state: PpuStartupState) {
        self.lcdc = startup_state.lcdc;
        self.stat_interrupt_enable = startup_state.stat & STAT_WRITABLE_ENABLE_MASK;
        self.mode = PpuAccessMode::from_stat_bits(startup_state.stat);
        self.scy = startup_state.scy;
        self.scx = startup_state.scx;
        self.ly = startup_state.ly;
        self.lyc = startup_state.lyc;
        self.bgp = startup_state.bgp;
        self.wy = startup_state.wy;
        self.wx = startup_state.wx;
        self.obp0 = None;
        self.obp1 = None;
        self.obj_palette_read_policy = startup_state.obj_palette_read_policy;
    }

    pub fn snapshot(&self) -> PpuSnapshot {
        PpuSnapshot {
            console_model: self.console_model,
            status: self.status,
            lcdc: self.lcdc,
            stat_interrupt_enable: self.stat_interrupt_enable,
            mode: self.mode,
            scy: self.scy,
            scx: self.scx,
            ly: self.ly,
            lyc: self.lyc,
            bgp: self.bgp,
            obp0: self.obp0,
            obp1: self.obp1,
            wy: self.wy,
            wx: self.wx,
            obj_palette_read_policy: self.obj_palette_read_policy,
        }
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        format!(
            "t_cycle={} phase={} console_model={:?} status={:?}",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
        )
    }

    fn is_lcd_enabled(&self) -> bool {
        self.lcdc & LCDC_ENABLE_BIT != 0
    }

    fn read_lcdc(&self) -> u8 {
        self.lcdc
    }

    fn write_lcdc(&mut self, value: u8) {
        self.lcdc = value;
    }

    fn read_stat(&self) -> u8 {
        STAT_FORCED_HIGH_BIT
            | self.stat_interrupt_enable
            | if self.ly == self.lyc { 0x04 } else { 0x00 }
            | if self.is_lcd_enabled() {
                self.mode.stat_bits()
            } else {
                PpuAccessMode::HBlank.stat_bits()
            }
    }

    fn write_stat(&mut self, value: u8) {
        self.stat_interrupt_enable = value & STAT_WRITABLE_ENABLE_MASK;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_state_recreates_the_documented_post_boot_lcd_snapshot() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);

        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x91,
            stat: 0x85,
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,
            lyc: 0x00,
            bgp: 0xFC,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        assert_eq!(ppu.read_register(0xFF40), 0x91);
        assert_eq!(ppu.read_register(0xFF41), 0x85);
        assert_eq!(ppu.read_register(0xFF42), 0x00);
        assert_eq!(ppu.read_register(0xFF43), 0x00);
        assert_eq!(ppu.read_register(0xFF44), 0x00);
        assert_eq!(ppu.read_register(0xFF45), 0x00);
        assert_eq!(ppu.read_register(0xFF47), 0xFC);
        assert_eq!(ppu.read_register(0xFF4A), 0x00);
        assert_eq!(ppu.read_register(0xFF4B), 0x00);
        assert_eq!(
            ppu.bus_state(),
            PpuBusState::lcd_enabled(PpuAccessMode::VBlank)
        );
    }

    #[test]
    fn stat_keeps_live_mode_and_coincidence_bits_outside_the_writable_mask() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x81,
            scy: 0x00,
            scx: 0x00,
            ly: 0x12,
            lyc: 0x12,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        ppu.write_register(0xFF41, 0xFF);

        assert_eq!(ppu.read_register(0xFF41), 0xFD);
    }

    #[test]
    fn ly_is_read_only_and_obj_palettes_keep_an_explicit_uninitialized_policy() {
        let mut ppu = Ppu::new(ConsoleModel::Dmg);
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x91,
            stat: 0x85,
            scy: 0x00,
            scx: 0x00,
            ly: 0x22,
            lyc: 0x00,
            bgp: 0xFC,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });

        ppu.write_register(0xFF44, 0x99);

        assert_eq!(ppu.read_register(0xFF44), 0x22);
        assert_eq!(ppu.read_register(0xFF48), 0xFF);
        assert_eq!(ppu.read_register(0xFF49), 0xFF);
    }
}
