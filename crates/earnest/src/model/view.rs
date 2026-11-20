/// The statechart labels of a model: implemented by `#[view(...)]` on a second `impl` block.
///
/// `earnest chart` and `earnest report` read the labels from the file, and a model that has
/// no view is drawn with `field = value` boxes and each action's sentence. This trait gives a
/// test the same labels: `state.state_label()` is what the chart writes in the state's box.
///
/// The conformance model `tasks/conformance/twins/cart.rs` is the Rust twin of the TypeScript
/// model with the same name and view:
///
/// ```
/// include!("../../../../tasks/conformance/twins/cart.rs");
///
/// use earnest::model::View;
///
/// fn main() {
///     let start = Cart::starts().remove(0);
///     assert_eq!(start.state_label().as_deref(), Some("Add, focus on Add"));
///     assert_eq!(start.group_label().as_deref(), Some("results"));
///     assert_eq!(Cart::title(), Some("Add to cart focus"));
///     assert_eq!(Cart::step_labels(), [("press", "press Add")]);
///     assert_eq!(Cart::colors(), [("broken", "#c62828")]);
/// }
/// ```
pub trait View: Sized {
    /// The chart's title, when the view gives one.
    fn title() -> Option<&'static str>;

    /// The label of this state's box, when the view has a `#[state]` method.
    fn state_label(&self) -> Option<String>;

    /// The label of the group this state is drawn in, when the view has a `#[group]` method.
    fn group_label(&self) -> Option<String>;

    /// Each `step(action = "...")` of the view as an action id and its arrow label. An action
    /// not listed uses its sentence.
    fn step_labels() -> &'static [(&'static str, &'static str)];

    /// Each `colors(part = "...")` of the view as a part (`broken`, `start` or `end`) and its
    /// color.
    fn colors() -> &'static [(&'static str, &'static str)];
}
