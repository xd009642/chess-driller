use anyhow::anyhow;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::EventPump;

pub struct EventSystem {
    pump: EventPump,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Event {
    /// Close the application
    should_close: bool,
    maybe_dragging: bool,
    is_dragging: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub kind: EventKind,
    pub timestamp: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    Close,
    /// Flips the board perspective
    FlipBoard,
    /// Reset the board
    Reset,
    /// Click somewhere on the screen
    MouseDown { x: i32, y: i32 },
    /// Start playing through your prep
    StartPractising,
    MouseUp { x: i32, y: i32 },
    MouseClick { x: i32, y: i32 },
    MouseDragBegin { x: i32, y: i32 },
    MouseDragMove { x: i32, y: i32 },
    MouseDragEnd { x: i32, y: i32 },
}

impl EventSystem {
    pub fn new(sdl: sdl2::Sdl) -> anyhow::Result<Self> {
        Ok(Self {
            pump: sdl.event_pump().map_err(|e| anyhow!(e))?,
            should_close: false,
            maybe_dragging: false,
            is_dragging: false,
        })
    }

    pub fn handle_events(&mut self) -> Vec<Event> {
        let mut events = vec![];

        for event in self.pump.poll_iter() {
            use sdl2::event::Event as SdlEvent;

            match &event {
                SdlEvent::MouseMotion { .. } => {}
                _ => self.maybe_dragging = false,
            }

            match event {
                SdlEvent::Quit { timestamp, .. } => {
                    events.push(Event {
                        kind: EventKind::Close,
                        timestamp,
                    });
                }
                SdlEvent::KeyUp {
                    keycode, timestamp, ..
                } => match keycode {
                    Some(Keycode::F) => {
                        events.push(Event {
                            kind: EventKind::FlipBoard,
                            timestamp,
                        });
                    }
                    Some(Keycode::R) => {
                        events.push(Event {
                            kind: EventKind::Reset,
                            timestamp,
                        });
                    }
                    Some(Keycode::Space) => {
                        events.push(Event::StartPractising);
                    }
                    _ => {
                        println!("Unsupported key: {:?}", keycode);
                    }
                },
                SdlEvent::MouseButtonUp {
                    x,
                    y,
                    mouse_btn,
                    timestamp,
                    ..
                } if mouse_btn == MouseButton::Left => {
                    if self.is_dragging {
                        self.is_dragging = false;
                        events.push(Event {
                            kind: EventKind::MouseDragEnd { x, y },
                            timestamp,
                        });
                        continue;
                    }

                    if let Some(Event {
                        kind: EventKind::MouseDown { x: x2, y: y2 },
                        timestamp: last_timestamp,
                    }) = events.last()
                    {
                        if timestamp - last_timestamp < 150
                            && x.abs_diff(*x2) < 5
                            && y.abs_diff(*y2) < 5
                        {
                            events.pop();
                            events.push(Event {
                                kind: EventKind::MouseClick { x, y },
                                timestamp,
                            });
                        }
                    } else {
                        events.push(Event {
                            kind: EventKind::MouseUp { x, y },
                            timestamp,
                        });
                    }
                }
                SdlEvent::MouseButtonDown {
                    x,
                    y,
                    mouse_btn,
                    timestamp,
                    ..
                } if mouse_btn == MouseButton::Left => {
                    events.push(Event {
                        kind: EventKind::MouseDown { x, y },
                        timestamp,
                    });
                }
                SdlEvent::MouseMotion {
                    timestamp, x, y, ..
                } => {
                    if self.is_dragging {
                        events.push(Event {
                            kind: EventKind::MouseDragMove { x, y },
                            timestamp,
                        });
                    } else if self.maybe_dragging {
                        events.push(Event {
                            kind: EventKind::MouseDragBegin { x, y },
                            timestamp,
                        });

                        self.maybe_dragging = false;
                        self.is_dragging = true;
                    } else {
                        events.push(Event {
                            kind: EventKind::MouseUp { x, y },
                            timestamp,
                        });
                    }
                }
                _e => {}
            }
        }
        // I'm lazy and start practising should be at the end to make sure we don't start playing
        // against our white prep as black
        events.sort();

        if let Some(Event {
            kind: EventKind::MouseDown { .. },
            ..
        }) = events.last()
        {
            self.maybe_dragging = true;
        }

        events
    }
}
