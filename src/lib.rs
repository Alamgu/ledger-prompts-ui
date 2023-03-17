#![no_std]
#![feature(cfg_version)]
#![cfg_attr(not(version("1.56")), feature(try_trait))]

use arrayvec::ArrayString;
use core::fmt::Write;
use ledger_log::trace;
use nanos_sdk::buttons::{ButtonEvent, ButtonsState};
use nanos_ui::bagls::*;
use nanos_ui::layout::*;
use nanos_ui::ui::{clear_screen, get_event, MessageValidator, SingleMessage};

pub mod bitmaps;

#[derive(Debug)]
pub struct PromptWrite<'a, const N: usize> {
    offset: usize,
    buffer: &'a mut ArrayString<N>,
    total: usize,
}

pub fn mk_prompt_write<'a, const N: usize>(buffer: &'a mut ArrayString<N>) -> PromptWrite<'a, N> {
    PromptWrite {
        offset: 0,
        buffer: buffer,
        total: 0,
    }
}

impl<'a, const N: usize> Write for PromptWrite<'a, N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.total += s.len();
        let offset_in_s = core::cmp::min(self.offset, s.len());
        self.offset -= offset_in_s;
        if self.offset > 0 {
            return Ok(());
        }
        let rv = self
            .buffer
            .try_push_str(
                &s[offset_in_s
                    ..core::cmp::min(s.len(), offset_in_s + self.buffer.remaining_capacity())],
            )
            .map_err(|_| core::fmt::Error);
        rv
    }
}

pub fn final_accept_prompt(prompt: &[&str]) -> Option<()> {
    if !MessageValidator::new(prompt, &[&"Confirm"], &[&"Reject"]).ask() {
        trace!("User rejected at end\n");
        None
    } else {
        trace!("User accepted");
        Some(())
    }
}

pub struct ScrollerError;
impl From<core::fmt::Error> for ScrollerError {
    fn from(_: core::fmt::Error) -> Self {
        ScrollerError
    }
}
impl From<core::str::Utf8Error> for ScrollerError {
    fn from(_: core::str::Utf8Error) -> Self {
        ScrollerError
    }
}

#[cfg(not(version("1.56")))]
impl From<core::option::NoneError> for ScrollerError {
    fn from(_: core::option::NoneError) -> Self {
        ScrollerError
    }
}

#[inline(never)]
pub fn write_scroller<F: for<'b> Fn(&mut PromptWrite<'b, 16>) -> Result<(), ScrollerError>>(
    show_index: bool,
    title: &str,
    prompt_function: F,
) -> Option<()> {
    if !WriteScroller::<_, 16>::new(title, prompt_function).ask(show_index) {
        trace!("User rejected prompt");
        None
    } else {
        Some(())
    }
}

#[inline(never)]
pub fn write_scroller_three_rows<
    F: for<'b> Fn(&mut PromptWrite<'b, 16>) -> Result<(), ScrollerError>,
>(
    show_index: bool,
    title: &str,
    prompt_function: F,
) -> Option<()> {
    if !WriteScroller::<_, 16>::new(title, prompt_function).ask_three_rows(show_index) {
        trace!("User rejected prompt");
        None
    } else {
        Some(())
    }
}

pub struct WriteScroller<
    'a,
    F: for<'b> Fn(&mut PromptWrite<'b, CHAR_N>) -> Result<(), ScrollerError>,
    const CHAR_N: usize,
> {
    title: &'a str,
    contents: F,
}

#[cfg(target_os = "nanos")]
const RIGHT_CHECK: Icon = Icon::new(Icons::Check).pos(120, 12);

#[cfg(not(target_os = "nanos"))]
const CHECK_ICON: Icon = Icon::from(&bitmaps::CHECK_GLYPH);
#[cfg(not(target_os = "nanos"))]
const RIGHT_CHECK: Icon = CHECK_ICON.shift_h(120);

impl<
        'a,
        F: for<'b> Fn(&mut PromptWrite<'b, CHAR_N>) -> Result<(), ScrollerError>,
        const CHAR_N: usize,
    > WriteScroller<'a, F, CHAR_N>
{
    pub fn new(title: &'a str, contents: F) -> Self {
        WriteScroller { title, contents }
    }

    fn get_length(&self) -> Result<usize, ScrollerError> {
        let mut buffer = ArrayString::new();
        let mut prompt_write = PromptWrite {
            offset: 0,
            buffer: &mut buffer,
            total: 0,
        };
        (self.contents)(&mut prompt_write)?;
        let length = prompt_write.total;
        trace!("Prompt length: {}", length);
        Ok(length)
    }

    pub fn ask(&self, show_index: bool) -> bool {
        self.ask_err(show_index).unwrap_or(false)
    }

    pub fn ask_err(&self, show_index: bool) -> Result<bool, ScrollerError> {
        let mut buttons = ButtonsState::new();
        let page_count = (core::cmp::max(1, self.get_length()?) - 1) / CHAR_N + 1;
        if page_count == 0 {
            return Ok(true);
        }
        if page_count > 1000 {
            trace!("Page count too large: {}", page_count);
            panic!("Page count too large: {}", page_count);
        }
        let mut cur_page = 0;

        // A closure to draw common elements of the screen
        // cur_page passed as parameter to prevent borrowing
        let draw = |page: usize| -> Result<(), ScrollerError> {
            clear_screen();
            let offset = page * CHAR_N;
            let mut buffer = ArrayString::new();
            (self.contents)(&mut PromptWrite {
                offset,
                buffer: &mut buffer,
                total: 0,
            })?;

            if show_index {
                let title_buffer = self.make_title_buffer(page, page_count);
                let title_label: Label = From::from(title_buffer.as_str());
                title_label.location(Location::Top).display();
            } else {
                let title_label: Label = From::from(self.title);
                title_label.location(Location::Top).display();
            };
            let label: Label = From::from(buffer.as_str());
            label.location(Location::Custom(15)).display();
            trace!(
                "Prompting with ({} of {}) {}: {}",
                page,
                page_count,
                self.title,
                buffer
            );
            if page > 0 {
                LEFT_ARROW.instant_display();
            }
            if page + 1 < page_count {
                RIGHT_ARROW.instant_display();
            } else {
                RIGHT_CHECK.instant_display();
            }
            Ok(())
        };

        draw(cur_page)?;

        loop {
            match get_event(&mut buttons) {
                Some(ButtonEvent::LeftButtonPress) => {
                    LEFT_S_ARROW.instant_display();
                }
                Some(ButtonEvent::RightButtonPress) => {
                    RIGHT_S_ARROW.instant_display();
                }
                Some(ButtonEvent::LeftButtonRelease) => {
                    if cur_page > 0 {
                        cur_page -= 1;
                    }
                    // We need to draw anyway to clear button press arrow
                    draw(cur_page)?;
                }
                Some(ButtonEvent::RightButtonRelease) => {
                    if cur_page < page_count {
                        cur_page += 1;
                    }
                    if cur_page == page_count {
                        break Ok(true);
                    }
                    // We need to draw anyway to clear button press arrow
                    draw(cur_page)?;
                }
                Some(ButtonEvent::BothButtonsRelease) => break Ok(false),
                Some(_) | None => (),
            }
        }
    }

    pub fn ask_three_rows(&self, show_index: bool) -> bool {
        self.ask_three_rows_err(show_index).unwrap_or(false)
    }

    pub fn ask_three_rows_err(&self, show_index: bool) -> Result<bool, ScrollerError> {
        let mut buttons = ButtonsState::new();
        let content_len = core::cmp::max(1, self.get_length()?);
        let page_count = (content_len - 1) / (CHAR_N * 3) + 1;
        if page_count == 0 {
            return Ok(true);
        }
        if page_count > 1000 {
            trace!("Page count too large: {}", page_count);
            panic!("Page count too large: {}", page_count);
        }
        let mut cur_page = 0;

        // A closure to draw common elements of the screen
        // cur_page passed as parameter to prevent borrowing
        let draw = |page: usize| -> Result<(), ScrollerError> {
            clear_screen();
            if show_index {
                let title_buffer = self.make_title_buffer(page, page_count);
                let title_label: Label = From::from(title_buffer.as_str());
                title_label.location(Location::Top).display();
            } else {
                let title_label: Label = From::from(self.title);
                title_label.location(Location::Top).display();
            };
            {
                let offset = (3 * page) * CHAR_N;
                let mut buffer = ArrayString::new();
                (self.contents)(&mut PromptWrite {
                    offset,
                    buffer: &mut buffer,
                    total: 0,
                })?;
                let label: Label = From::from(buffer.as_str());
                label.location(Location::Custom(16)).display();
                trace!(
                    "Prompting row 1 ({} of {}) {}: {}",
                    page,
                    page_count,
                    self.title,
                    buffer
                );
            }
            if content_len > ((3 * page) + 1) * CHAR_N {
                let offset = ((3 * page) + 1) * CHAR_N;
                let mut buffer = ArrayString::new();
                (self.contents)(&mut PromptWrite {
                    offset,
                    buffer: &mut buffer,
                    total: 0,
                })?;
                let label: Label = From::from(buffer.as_str());
                label.location(Location::Custom(31)).display();
                trace!(
                    "Prompting row 2 ({} of {}) {}: {}",
                    page,
                    page_count,
                    self.title,
                    buffer
                );
            }
            if content_len > ((3 * page) + 2) * CHAR_N {
                let offset = ((3 * page) + 2) * CHAR_N;
                let mut buffer = ArrayString::new();
                (self.contents)(&mut PromptWrite {
                    offset,
                    buffer: &mut buffer,
                    total: 0,
                })?;
                let label: Label = From::from(buffer.as_str());
                label.location(Location::Custom(46)).display();
                trace!(
                    "Prompting row 3 ({} of {}) {}: {}",
                    page,
                    page_count,
                    self.title,
                    buffer
                );
            }
            if page > 0 {
                LEFT_ARROW.instant_display();
            }
            if page + 1 < page_count {
                RIGHT_ARROW.instant_display();
            } else {
                RIGHT_CHECK.instant_display();
            }
            Ok(())
        };

        draw(cur_page)?;

        loop {
            match get_event(&mut buttons) {
                Some(ButtonEvent::LeftButtonPress) => {
                    LEFT_S_ARROW.instant_display();
                }
                Some(ButtonEvent::RightButtonPress) => {
                    RIGHT_S_ARROW.instant_display();
                }
                Some(ButtonEvent::LeftButtonRelease) => {
                    if cur_page > 0 {
                        cur_page -= 1;
                    }
                    // We need to draw anyway to clear button press arrow
                    draw(cur_page)?;
                }
                Some(ButtonEvent::RightButtonRelease) => {
                    if cur_page < page_count {
                        cur_page += 1;
                    }
                    if cur_page == page_count {
                        break Ok(true);
                    }
                    // We need to draw anyway to clear button press arrow
                    draw(cur_page)?;
                }
                Some(ButtonEvent::BothButtonsRelease) => break Ok(false),
                Some(_) | None => (),
            }
        }
    }

    fn make_title_buffer(&self, page: usize, page_count: usize) -> ArrayString<16> {
        let mut title_buffer: ArrayString<16> = ArrayString::new();
        // Number of chars needed to show " (x/y)"
        let len_needed = 4 + if page_count < 10 {
            2
        } else if page_count < 100 {
            4
        } else {
            6
        };

        title_buffer.push_str(self.title);
        if page_count > 1 && self.title.len() <= (16 - len_needed) {
            // We have checked that the following will succeed, so ignore result
            let _ = write!(
                mk_prompt_write(&mut title_buffer),
                " ({}/{})",
                page + 1,
                page_count
            );
        }
        title_buffer
    }
}

pub trait Menu {
    type BothResult;
    fn move_left(&mut self);
    fn move_right(&mut self);
    fn handle_both(&mut self) -> Option<Self::BothResult>;
    fn label(&self) -> &str;
}

#[inline(never)]
pub fn show_menu<M: Menu>(menu: &M) {
    SingleMessage::new(menu.label()).show();
}

#[inline(never)]
pub fn handle_menu_button_event<M: Menu>(menu: &mut M, btn: ButtonEvent) -> Option<<M as Menu>::BothResult> {
    match btn {
        ButtonEvent::LeftButtonRelease => {
            menu.move_left();
        }
        ButtonEvent::RightButtonRelease => {
            menu.move_right();
        }
        ButtonEvent::BothButtonsRelease => {
            return menu.handle_both()
        },
        _ => (),
    }
    None
}
