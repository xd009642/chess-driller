use anyhow::anyhow;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::EventPump;

pub struct EventSystem {
    pump: EventPump,
    should_close: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Close,
    FlipBoard,
    Reset,
    MouseDown { x: i32, y: i32 },
}

impl EventSystem {
    pub fn new(sdl: sdl2::Sdl) -> anyhow::Result<Self> {
        Ok(Self {
            pump: sdl.event_pump().map_err(|e| anyhow!(e))?,
            should_close: false,
        })
    }

    pub fn handle_events(&mut self) -> Vec<Event> {
        let mut events = vec![];
        for event in self.pump.poll_iter() {
            use sdl2::event::Event as SdlEvent;
            match event {
                SdlEvent::Quit { .. } => {
                    events.push(Event::Close);
                }
                SdlEvent::KeyUp { keycode, .. } => match keycode {
                    Some(Keycode::F) => {
                        events.push(Event::FlipBoard);
                    }
                    Some(Keycode::R) => {
                        events.push(Event::Reset);
                    }
                    _ => {
                        println!("Unsupported key: {:?}", keycode);
                    }
                },
                SdlEvent::MouseButtonDown {
                    x, y, mouse_btn, ..
                } if mouse_btn == MouseButton::Left => {
                    events.push(Event::MouseDown { x, y });
                }
                _e => {}
            }
        }
        events
    }
}
