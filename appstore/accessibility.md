# App Accessibility  (App Store → Trust & Safety → App Accessibility)

**Optional.** Accessibility Nutrition Labels are not required to submit, and an
absent label is not held against you.

---

## Recommendation: leave it blank for 0.1.0

Not out of laziness — because a claim here is a promise, and an inaccurate one is
worse than no label at all. Apple's labels declare *supported* features, and
users relying on assistive technology act on them.

What TimeBox actually has today, from the source:

- 9 `aria-label` attributes, plus `role="switch"`, `role="button"`,
  `aria-pressed`, `aria-checked` and `aria-invalid`
- keyboard shortcuts across 11 handlers, and 4 explicit focus calls

That is a reasonable foundation and better than nothing. But it has **never been
tested with VoiceOver**, there is no verified focus order through the checkpoint,
and the countdown does not announce changes. Ticking "VoiceOver" on that basis
would be a guess.

---

## What to do before claiming anything

1. Turn on VoiceOver (Cmd+F5) and drive the whole flow with the keyboard alone:
   add a task, start a block, reach the checkpoint, choose an option.
2. Confirm the checkpoint traps focus and announces its options — it is the one
   screen with no escape, so it is the one that must be navigable.
3. Check contrast in both light and dark themes.

Then fill the label in honestly for the version that ships those fixes.
