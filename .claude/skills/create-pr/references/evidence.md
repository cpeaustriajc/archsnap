# Screenshots and video for the Evidence line

Evidence shows before and after of the same state. A visual change gets a screenshot
cropped to the change. A flow, a motion or a timing also gets a short video. Each video
and screenshot:
- shows where each click lands and which key was pressed, with no test code on screen.
  Use the project's helper when it has one, else the recorder's own marks
  (`references/<recorder>.md`).
- plays at a person's pace: the hero script pauses before each action, types at human
  speed and holds where the viewer reads.
  Failure: a 16-second clip of two whole flows that a reviewer could not follow.
- starts after sign-in and setup and ends before cleanup, so it shows only the user's
  steps.
- comes from a hero script: a standalone script made only for the video, run outside the
  test runner, that reuses the tests' setup helpers. A traced test run fails the fill check of
  `scripts/check-video.sh`.
- opens each step with a chapter card and reaches each page by the click a person would
  make, never a URL jump. Without both, the video jumps from page to page and the viewer gets lost.
- closes on screen each tool it opens (a dev panel, a QA menu) before the user's next
  step, so no state changes without a visible cause.

Before you attach a video, run `scripts/check-video.sh <file> <MB>` with the upload limit and
read each WARN still: a chapter card is fine, a frozen step is not. Then read a contact sheet of
its frames for the marks. Upload each file to the code host and reference it in the body by its
path relative to the body file. Put a video alone in its paragraph.
