use crate::cartridge::CartridgeHeader;
use crate::model::SgbHostProfile;

mod attribute;
mod boot_palette;
mod border_data;
mod color;
mod constants;
mod obj;
mod packet;
mod palette;
mod requests;
mod transfer;

pub use self::attribute::*;
pub use self::border_data::*;
pub use self::color::*;
pub use self::constants::*;
pub use self::obj::*;
pub use self::packet::*;
pub use self::palette::*;
pub use self::requests::*;
pub use self::transfer::*;

#[allow(unused_imports)]
pub(in crate::sgb) use self::{
    attribute::*, boot_palette::*, border_data::*, color::*, constants::*, obj::*, packet::*,
    palette::*, requests::*, transfer::*,
};
