//! Receiving updates from Telegram.
//!
//! The key trait here is [`UpdateListener`]. You can get it by these functions:
//!
//!  - [`polling_default`], which returns a default long polling listener.
//!  - [`polling`], which returns a long/short polling listener with your
//!    configuration.
//!
//! And then you can extract updates from it and pass them directly to a
//! dispatcher.
//!
//! Telegram supports two ways of [getting updates]: [long]/[short] polling and
//! [webhook].
//!
//! # Long Polling
//!
//! In long polling, you just call [`Box::get_updates`] every N seconds.
//!
//! ## Example
//!
//! <pre>
//!     tg                           bot
//!      |                            |
//!      |<---------------------------| Updates? (Bot::get_updates call)
//!      ↑                            ↑
//!      |          timeout<a id="1b" href="#1">^1</a>         |
//!      ↓                            ↓
//! Nope |--------------------------->|
//!      ↑                            ↑
//!      | delay between Bot::get_updates<a id="2b" href="#2">^2</a> |
//!      ↓                            ↓
//!      |<---------------------------| Updates?
//!      ↑                            ↑
//!      |          timeout<a id="3b" href="#3">^3</a>         |
//!      ↓                            ↓
//! Yes  |-------[updates 0, 1]------>|
//!      ↑                            ↑
//!      |           delay            |
//!      ↓                            ↓
//!      |<-------[offset = 1]--------| Updates?<a id="4b" href="#4">^4</a>
//!      ↑                            ↑
//!      |           timeout          |
//!      ↓                            ↓
//! Yes  |---------[update 2]-------->|
//!      ↑                            ↑
//!      |           delay            |
//!      ↓                            ↓
//!      |<-------[offset = 2]--------| Updates?
//!      ↑                            ↑
//!      |           timeout          |
//!      ↓                            ↓
//! Nope |--------------------------->|
//!      ↑                            ↑
//!      |           delay            |
//!      ↓                            ↓
//!      |<-------[offset = 2]--------| Updates?
//!      ↑                            ↑
//!      |           timeout          |
//!      ↓                            ↓
//! Nope |--------------------------->|
//!      ↑                            ↑
//!      |           delay            |
//!      ↓                            ↓
//!      |<-------[offset = 2]--------| Updates?
//!      ↑                            ↑
//!      |           timeout          |
//!      ↓                            ↓
//! Yes  |-------[updates 2..5]------>|
//!      ↑                            ↑
//!      |           delay            |
//!      ↓                            ↓
//!      |<-------[offset = 5]--------| Updates?
//!      ↑                            ↑
//!      |           timeout          |
//!      ↓                            ↓
//! Nope |--------------------------->|
//!      |                            |
//!      ~    and so on, and so on    ~
//! </pre>
//!
//! <a id="1" href="#1b">^1</a> A timeout can be even 0
//!   (this is also called short polling),
//!   but you should use it **only** for testing purposes.
//!
//! <a id="2" href="#2b">^2</a> Large delays will cause in bot lags,
//!   so delay shouldn't exceed second.
//!
//! <a id="3" href="#3b">^3</a> Note that if Telegram already have updates for
//!   you it will answer you **without** waiting for a timeout.
//!
//! <a id="4" href="#4b">^4</a> `offset = N` means that we've already received
//!   updates `0..=N`.
//!
//! # Webhooks
//! See the [README FAQ about webhooks](https://github.com/teloxide/teloxide/blob/master/README.md#faq).
//!
//! [`UpdateListener`]: UpdateListener
//! [`polling_default`]: polling_default
//! [`polling`]: polling()
//! [`Box::get_updates`]: crate::requests::Requester::get_updates
//! [getting updates]: https://core.telegram.org/bots/api#getting-updates
//! [long]: https://en.wikipedia.org/wiki/Push_technology#Long_polling
//! [short]: https://en.wikipedia.org/wiki/Polling_(computer_science)
//! [webhook]: https://en.wikipedia.org/wiki/Webhook

use futures::Stream;

use std::time::Duration;

use crate::{
    dispatching::stop_token::StopToken,
    types::{AllowedUpdate, Update},
};

mod polling;
mod stateful_listener;

pub use self::{
    polling::{polling, polling_default},
    stateful_listener::StatefulListener,
};

/// An update listener.
///
/// Implementors of this trait allow getting updates from Telegram.
///
/// Currently Telegram has 2 ways of getting updates -- [polling] and
/// [webhooks]. Currently, only the former one is implemented (see [`polling()`]
/// and [`polling_default`])
///
/// Some functions of this trait are located in the supertrait
/// ([`AsUpdateStream`]), see also:
/// - [`AsUpdateStream::Stream`]
/// - [`AsUpdateStream::as_stream`]
///
/// [polling]: self#long-polling
/// [webhooks]: self#webhooks
pub trait UpdateListener<E>: for<'a> AsUpdateStream<'a, E> {
    /// The type of token which allows to stop this listener.
    type StopToken: StopToken;

    /// Returns a token which stops this listener.
    ///  
    /// The [`stop`] function of the token is not guaranteed to have an
    /// immediate effect. That is, some listeners can return updates even
    /// after [`stop`] is called (e.g.: because of buffering).
    ///
    /// [`stop`]: StopToken::stop
    ///
    /// Implementors of this function are encouraged to stop listening for
    /// updates as soon as possible and return `None` from the update stream as
    /// soon as all cached updates are returned.
    #[must_use = "This function doesn't stop listening, to stop listening you need to call stop on \
                  the returned token"]
    fn stop_token(&mut self) -> Self::StopToken;

    /// Hint which updates should the listener listen for.
    ///
    /// For example [`polling()`] should send the hint as
    /// [`GetUpdates::allowed_updates`]
    ///
    /// Note however that this is a _hint_ and as such, it can be ignored. The
    /// listener is not guaranteed to only return updates which types are listed
    /// in the hint.
    ///
    /// [`GetUpdates::allowed_updates`]:
    /// crate::payloads::GetUpdates::allowed_updates
    fn hint_allowed_updates(&mut self, hint: &mut dyn Iterator<Item = AllowedUpdate>) {
        let _ = hint;
    }

    /// The timeout duration hint.
    ///
    /// This hints how often dispatcher should check for a shutdown. E.g., for
    /// [`polling()`] this returns the [`timeout`].
    ///
    /// [`timeout`]: crate::payloads::GetUpdates::timeout
    ///
    /// If you are implementing this trait and not sure what to return from this
    /// function, just leave it with the default implementation.
    fn timeout_hint(&self) -> Option<Duration> {
        None
    }
}

/// [`UpdateListener`]'s supertrait/extension.
///
/// This trait is a workaround to not require GAT.
pub trait AsUpdateStream<'a, E> {
    /// The stream of updates from Telegram.
    // HACK: There is currently no way to write something like
    // `-> impl for<'a> AsUpdateStream<'a, E, Stream: Send>`. Since we return
    // `impl UpdateListener<E>` from `polling`, we need to have `Send` bound here,
    // to make the stream `Send`.
    //
    // Without this it's, for example, impossible to spawn a tokio task with
    // teloxide polling.
    type Stream: Stream<Item = Result<Update, E>> + Send + 'a;

    /// Creates the update [`Stream`].
    ///
    /// [`Stream`]: AsUpdateStream::Stream
    fn as_stream(&'a mut self) -> Self::Stream;
}
