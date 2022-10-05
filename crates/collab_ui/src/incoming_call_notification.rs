use call::ActiveCall;
use client::{incoming_call::IncomingCall, UserStore};
use futures::StreamExt;
use gpui::{
    elements::*,
    geometry::{rect::RectF, vector::vec2f},
    impl_internal_actions, Entity, ModelHandle, MouseButton, MutableAppContext, RenderContext,
    View, ViewContext, WindowBounds, WindowKind, WindowOptions,
};
use settings::Settings;
use util::ResultExt;
use workspace::JoinProject;

impl_internal_actions!(incoming_call_notification, [RespondToCall]);

pub fn init(user_store: ModelHandle<UserStore>, cx: &mut MutableAppContext) {
    cx.add_action(IncomingCallNotification::respond_to_call);

    let mut incoming_call = user_store.read(cx).incoming_call();
    cx.spawn(|mut cx| async move {
        let mut notification_window = None;
        while let Some(incoming_call) = incoming_call.next().await {
            if let Some(window_id) = notification_window.take() {
                cx.remove_window(window_id);
            }

            if let Some(incoming_call) = incoming_call {
                let (window_id, _) = cx.add_window(
                    WindowOptions {
                        bounds: WindowBounds::Fixed(RectF::new(vec2f(0., 0.), vec2f(300., 400.))),
                        titlebar: None,
                        center: true,
                        kind: WindowKind::PopUp,
                        is_movable: false,
                    },
                    |_| IncomingCallNotification::new(incoming_call, user_store.clone()),
                );
                notification_window = Some(window_id);
            }
        }
    })
    .detach();
}

#[derive(Clone, PartialEq)]
struct RespondToCall {
    accept: bool,
}

pub struct IncomingCallNotification {
    call: IncomingCall,
    user_store: ModelHandle<UserStore>,
}

impl IncomingCallNotification {
    pub fn new(call: IncomingCall, user_store: ModelHandle<UserStore>) -> Self {
        Self { call, user_store }
    }

    fn respond_to_call(&mut self, action: &RespondToCall, cx: &mut ViewContext<Self>) {
        if action.accept {
            let join = ActiveCall::global(cx)
                .update(cx, |active_call, cx| active_call.join(&self.call, cx));
            let caller_user_id = self.call.caller.id;
            let initial_project_id = self.call.initial_project_id;
            cx.spawn_weak(|_, mut cx| async move {
                join.await?;
                if let Some(project_id) = initial_project_id {
                    cx.update(|cx| {
                        cx.dispatch_global_action(JoinProject {
                            project_id,
                            follow_user_id: caller_user_id,
                        })
                    });
                }
                anyhow::Ok(())
            })
            .detach_and_log_err(cx);
        } else {
            self.user_store
                .update(cx, |user_store, _| user_store.decline_call().log_err());
        }

        let window_id = cx.window_id();
        cx.remove_window(window_id);
    }

    fn render_caller(&self, cx: &mut RenderContext<Self>) -> ElementBox {
        let theme = &cx.global::<Settings>().theme.contacts_panel;
        Flex::row()
            .with_children(
                self.call
                    .caller
                    .avatar
                    .clone()
                    .map(|avatar| Image::new(avatar).with_style(theme.contact_avatar).boxed()),
            )
            .with_child(
                Label::new(
                    self.call.caller.github_login.clone(),
                    theme.contact_username.text.clone(),
                )
                .boxed(),
            )
            .boxed()
    }

    fn render_buttons(&self, cx: &mut RenderContext<Self>) -> ElementBox {
        enum Accept {}
        enum Decline {}

        Flex::row()
            .with_child(
                MouseEventHandler::<Accept>::new(0, cx, |_, cx| {
                    let theme = &cx.global::<Settings>().theme.contacts_panel;
                    Label::new("Accept".to_string(), theme.contact_username.text.clone()).boxed()
                })
                .on_click(MouseButton::Left, |_, cx| {
                    cx.dispatch_action(RespondToCall { accept: true });
                })
                .boxed(),
            )
            .with_child(
                MouseEventHandler::<Decline>::new(0, cx, |_, cx| {
                    let theme = &cx.global::<Settings>().theme.contacts_panel;
                    Label::new("Decline".to_string(), theme.contact_username.text.clone()).boxed()
                })
                .on_click(MouseButton::Left, |_, cx| {
                    cx.dispatch_action(RespondToCall { accept: false });
                })
                .boxed(),
            )
            .boxed()
    }
}

impl Entity for IncomingCallNotification {
    type Event = ();
}

impl View for IncomingCallNotification {
    fn ui_name() -> &'static str {
        "IncomingCallNotification"
    }

    fn render(&mut self, cx: &mut RenderContext<Self>) -> gpui::ElementBox {
        Flex::column()
            .with_child(self.render_caller(cx))
            .with_child(self.render_buttons(cx))
            .boxed()
    }
}
