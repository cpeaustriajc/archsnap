use super::Start;
use std::borrow::Borrow;
use std::fmt;

/// Messages waiting in the order they were sent, meant to hold at most `CAPACITY`. As a model
/// field it starts empty.
///
/// Sending to a full queue is not an error here: the value just grows, and the checker
/// reports the state as out of range. Guard the send with `!queue.is_full()` to rule it out.
/// `Debug` prints the messages head first: `[Bill, Card]`.
///
/// The conformance model `tasks/conformance/models/mailbox.rs` holds a queue of two letters.
/// Its rule `bills` breaks after `deliver[letter = "Bill"]` twice:
///
/// ```
/// include!("../../../../tasks/conformance/models/mailbox.rs");
///
/// fn main() {
///     let starts = Mailbox::starts();
///     assert_eq!(starts.len(), 1);
///     let start = &starts[0];
///     assert_eq!(format!("{start:?}"), "Mailbox { inbox: [] }");
///     assert_eq!(start.read(), None);
///     assert_eq!(start.empty(), None);
///
///     let one = start.deliver(Letter::Bill).expect("an empty inbox has room");
///     assert_eq!(format!("{one:?}"), "Mailbox { inbox: [Bill] }");
///     assert!(one.bills() && one.fronted());
///
///     let two = one.deliver(Letter::Bill).expect("an inbox of one has room");
///     assert_eq!(format!("{two:?}"), "Mailbox { inbox: [Bill, Bill] }");
///     assert!(!two.bills());
///     assert!(two.fronted());
///     assert_eq!(two.deliver(Letter::Card), None);
///
///     assert_eq!(two.read().expect("a full inbox can be read"), one);
///     assert_eq!(&two.empty().expect("a full inbox can be emptied"), start);
/// }
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Queue<T, const CAPACITY: usize> {
    messages: Vec<T>,
}

impl<T: fmt::Debug, const CAPACITY: usize> fmt::Debug for Queue<T, CAPACITY> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(&self.messages).finish()
    }
}

impl<T, const CAPACITY: usize> Default for Queue<T, CAPACITY> {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
        }
    }
}

impl<T, const CAPACITY: usize> Start for Queue<T, CAPACITY> {
    fn starts() -> Vec<Self> {
        vec![Self::default()]
    }
}

impl<T, const CAPACITY: usize> Queue<T, CAPACITY> {
    pub(super) fn walked(&self) -> impl ExactSizeIterator<Item = &T> {
        self.messages.iter()
    }
}

impl<T: Clone + PartialEq, const CAPACITY: usize> Queue<T, CAPACITY> {
    /// How many messages are waiting.
    pub fn size(&self) -> usize {
        self.messages.len()
    }

    /// Whether no message is waiting.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Whether `CAPACITY` messages or more are waiting.
    pub fn is_full(&self) -> bool {
        self.messages.len() >= CAPACITY
    }

    /// The message that was sent first and is still waiting, or `None` when the queue is
    /// empty.
    pub fn head(&self) -> Option<T> {
        self.messages.first().cloned()
    }

    /// This queue with `message` added behind the others.
    #[must_use]
    pub fn send(&self, message: T) -> Self {
        let mut messages = self.messages.clone();
        messages.push(message);
        Self { messages }
    }

    /// This queue without its head. An empty queue stays empty. Read the head with
    /// [`Queue::head`] first when the message matters.
    #[must_use]
    pub fn receive(&self) -> Self {
        Self {
            messages: self.messages.iter().skip(1).cloned().collect(),
        }
    }

    /// Whether `message` is waiting anywhere in the queue.
    pub fn includes(&self, message: impl Borrow<T>) -> bool {
        self.messages.contains(message.borrow())
    }

    /// The empty queue of the same capacity.
    #[must_use]
    pub fn clear(&self) -> Self {
        Self::default()
    }

    /// Whether `test` is true for every waiting message. True for the empty queue.
    pub fn every(&self, test: impl Fn(T) -> bool) -> bool {
        self.messages.iter().cloned().all(test)
    }

    /// Whether `test` is true for at least one waiting message. False for the empty queue.
    pub fn some(&self, test: impl Fn(T) -> bool) -> bool {
        self.messages.iter().cloned().any(test)
    }
}
