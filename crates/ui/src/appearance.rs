//! GitHub Desktop-inspired light surfaces and compact native controls.
use iced::widget::{button, container};
use iced::{Background, Border, Color, Theme};

pub(super) const SIDEBAR: f32 = 292.0;
pub(super) const TEXT: Color = Color::from_rgb(31.0 / 255.0, 35.0 / 255.0, 40.0 / 255.0);
pub(super) const MUTED: Color = Color::from_rgb(101.0 / 255.0, 109.0 / 255.0, 118.0 / 255.0);
pub(super) const BORDER: Color = Color::from_rgb(208.0 / 255.0, 215.0 / 255.0, 222.0 / 255.0);
pub(super) const BLUE: Color = Color::from_rgb(9.0 / 255.0, 105.0 / 255.0, 218.0 / 255.0);
pub(super) const TOOLBAR: Color = Color::from_rgb(36.0 / 255.0, 41.0 / 255.0, 47.0 / 255.0);
pub(super) const SIDEBAR_BG: Color = Color::from_rgb(246.0 / 255.0, 248.0 / 255.0, 250.0 / 255.0);
pub(super) const ADDED: Color = Color::from_rgb(230.0 / 255.0, 1.0, 237.0 / 255.0);
pub(super) const DELETED: Color = Color::from_rgb(1.0, 235.0 / 255.0, 233.0 / 255.0);

pub(super) fn theme(_: &crate::Desktop) -> Theme {
    Theme::custom_with_fn(
        "Desktop light".into(),
        iced::theme::Palette {
            background: Color::WHITE,
            text: TEXT,
            primary: BLUE,
            success: Color::from_rgb8(26, 127, 55),
            danger: Color::from_rgb8(207, 34, 46),
        },
        |palette| {
            let mut extended = iced::theme::palette::Extended::generate(palette);
            extended.primary.strong.color = BLUE;
            extended.primary.strong.text = Color::WHITE;
            extended.primary.base.color = Color::from_rgb8(8, 96, 202);
            extended.primary.base.text = Color::WHITE;
            extended
        },
    )
}

pub(super) fn surface(color: Color) -> container::Style {
    container::Style {
        background: Some(color.into()),
        text_color: Some(TEXT),
        ..Default::default()
    }
}

pub(super) fn pane(_: &Theme) -> container::Style {
    container::Style {
        border: Border {
            color: BORDER,
            width: 1.0,
            ..Default::default()
        },
        ..surface(Color::WHITE)
    }
}

pub(super) fn sidebar(_: &Theme) -> container::Style {
    container::Style {
        border: Border {
            color: BORDER,
            width: 1.0,
            ..Default::default()
        },
        ..surface(SIDEBAR_BG)
    }
}

pub(super) fn toolbar_button(_: &Theme, status: button::Status) -> button::Style {
    let color = if status == button::Status::Hovered {
        Color::from_rgb8(48, 54, 61)
    } else {
        TOOLBAR
    };
    button::Style {
        background: Some(color.into()),
        text_color: Color::WHITE,
        border: Border {
            color: Color::from_rgb8(68, 76, 86),
            width: 1.0,
            ..Default::default()
        },
        ..Default::default()
    }
}

pub(super) fn tab(_: &Theme, status: button::Status, active: bool) -> button::Style {
    let background = if active {
        Color::WHITE
    } else if status == button::Status::Hovered {
        Color::from_rgb8(234, 238, 242)
    } else {
        SIDEBAR_BG
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: if active { BLUE } else { MUTED },
        border: Border {
            radius: 0.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub(super) fn file(_: &Theme, status: button::Status, active: bool) -> button::Style {
    let background = if active {
        BLUE
    } else if status == button::Status::Hovered {
        Color::from_rgb8(234, 238, 242)
    } else {
        Color::WHITE
    };
    button::Style {
        background: Some(background.into()),
        text_color: if active { Color::WHITE } else { TEXT },
        border: Border::default(),
        ..Default::default()
    }
}
