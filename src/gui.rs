use iced::Length::FillPortion;
use iced::task::Task;
use iced::widget::{button, column, container, row, text};
use iced::{Center, Element, Fill};
use std::cell::RefCell;
use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::{AppMessage, ScanOp};

pub fn run_gui(
    tx_to_app: std::sync::mpsc::Sender<AppMessage>,
    rx_from_app: iced::futures::channel::mpsc::Receiver<AppMessage>,
) -> iced::Result {
    let task = Task::stream(rx_from_app);

    let once_boot = RefCell::new(Some((Frontend::new(tx_to_app), task)));
    let boot = move || once_boot.borrow_mut().take().unwrap();
    iced::application(boot, Frontend::update, Frontend::view)
        .theme(iced::Theme::Dracula)
        .run()
}

pub struct Frontend {
    tx_to_app: Sender<AppMessage>,
    op: ScanOp,
    toast: Option<String>,
}

impl Frontend {
    fn new(tx_to_app: Sender<AppMessage>) -> Self {
        Frontend {
            op: ScanOp::None,
            tx_to_app,
            toast: None,
        }
    }

    fn update(&mut self, message: AppMessage) -> Task<AppMessage> {
        match message {
            AppMessage::GuiOpChange(mut op) => {
                if self.op == op {
                    op = ScanOp::None;
                }
                self.tx_to_app.send(AppMessage::GuiOpChange(op)).unwrap();
                self.op = op;
                Task::none()
            }
            AppMessage::Toast(msg) => {
                self.toast = Some(msg);
                Task::perform(tokio::time::sleep(Duration::from_secs(3)), |_| {
                    AppMessage::ClearToast
                })
            }
            AppMessage::ClearToast => {
                self.toast = None;
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn view(&self) -> Element<'_, AppMessage> {
        let op_style = |op| {
            if self.op == op {
                button::success
            } else {
                button::secondary
            }
        };

        let buttons = container(
            row![
                button(text("Add").size(100).align_x(Center).align_y(Center))
                    .style(op_style(ScanOp::Add))
                    .width(FillPortion(1))
                    .height(100)
                    .on_press_with(|| AppMessage::GuiOpChange(ScanOp::Add)),
                button(text("Remove").size(100).align_x(Center).align_y(Center))
                    .style(op_style(ScanOp::Remove))
                    .width(FillPortion(1))
                    .height(100)
                    .on_press_with(|| AppMessage::GuiOpChange(ScanOp::Remove)),
            ]
            .spacing(20),
        )
        .width(Fill);

        let buttons2 = container(
            row![
                button(text("Open").size(100).align_x(Center).align_y(Center))
                    .style(op_style(ScanOp::Open))
                    .width(FillPortion(1))
                    .height(100)
                    .on_press_with(|| AppMessage::GuiOpChange(ScanOp::Open)),
                button(text("Finish").size(100).align_x(Center).align_y(Center))
                    .style(op_style(ScanOp::Finish))
                    .width(FillPortion(1))
                    .height(100)
                    .on_press_with(|| AppMessage::GuiOpChange(ScanOp::Finish)),
            ]
            .spacing(20),
        )
        .width(Fill);

        let text = text(
            self.toast
                .clone()
                .unwrap_or_else(|| format!("{:?}", self.op)),
        )
        .size(50);

        column![buttons, buttons2, text]
            .width(Fill)
            .align_x(Center)
            .spacing(20)
            .padding(20)
            .into()
    }
}
