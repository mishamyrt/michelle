use gpui::{hsla, rgb};

use super::{FillColors, LabelColors, Palette, SeparatorColors, SystemColors};

pub(super) fn palette() -> Palette {
    Palette {
        labels: LabelColors {
            primary: rgb(0xE2E2E2).into(),
            secondary: rgb(0xA3A3A3).into(),
            tertiary: rgb(0x7D7D7D).into(),
            quaternary: rgb(0x575757).into(),
        },
        fills: FillColors {
            primary: hsla(220.0 / 360.0, 0.10, 0.90, 0.09),
            secondary: hsla(220.0 / 360.0, 0.10, 0.90, 0.08),
            tertiary: hsla(0.0, 0.0, 0.941, 0.06),
            quaternary: hsla(220.0 / 360.0, 0.10, 0.90, 0.05),
        },
        separators: SeparatorColors {
            standard: hsla(220.0 / 360.0, 0.10, 0.90, 0.07),
            strong: hsla(220.0 / 360.0, 0.10, 0.90, 0.11),
        },
        colors: SystemColors {
            blue: rgb(0x0A99FF).into(),
            blue_deep: rgb(0x3B79FF).into(),
            green: rgb(0x62C987).into(),
            green_bright: rgb(0x82E087).into(),
            orange: rgb(0xE0B36A).into(),
            yellow: rgb(0xEAB308).into(),
            red: rgb(0xE2726A).into(),
        },
    }
}
