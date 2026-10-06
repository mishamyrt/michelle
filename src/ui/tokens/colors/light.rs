use gpui::{hsla, rgb};

use super::{FillColors, LabelColors, Palette, SeparatorColors, SystemColors};

pub(super) fn palette() -> Palette {
    Palette {
        labels: LabelColors {
            primary: hsla(0.0, 0.0, 0.0, 0.85),
            secondary: hsla(0.0, 0.0, 0.0, 0.5),
            tertiary: hsla(0.0, 0.0, 0.0, 0.25),
            quaternary: hsla(0.0, 0.0, 0.0, 0.10),
        },
        fills: FillColors {
            primary: hsla(220.0 / 360.0, 0.10, 0.12, 0.09),
            secondary: hsla(220.0 / 360.0, 0.10, 0.12, 0.07),
            tertiary: hsla(0.0, 0.0, 0.078, 0.06),
            quaternary: hsla(220.0 / 360.0, 0.10, 0.12, 0.05),
        },
        separators: SeparatorColors {
            standard: hsla(0.0, 0.0, 0.0, 0.08),
            strong: hsla(220.0 / 360.0, 0.10, 0.12, 0.14),
        },
        colors: SystemColors {
            blue: rgb(0x0088FF).into(),
            blue_deep: rgb(0x2563EB).into(),
            green: rgb(0x2F8F52).into(),
            green_bright: rgb(0x289A2E).into(),
            orange: rgb(0xA66B20).into(),
            yellow: rgb(0xCA8A04).into(),
            red: rgb(0xC64A42).into(),
        },
    }
}
