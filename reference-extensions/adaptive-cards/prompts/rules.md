# Adaptive Cards v1.6 — Authoring Rules

- Top-level object MUST have `"type": "AdaptiveCard"` and `"version": "1.6"` (or earlier minor).
- Body items use element types: `TextBlock`, `Image`, `Container`, `ColumnSet`, `FactSet`, `ActionSet`, `Input.Text`, `Input.Number`, `Input.Date`, `Input.Time`, `Input.Toggle`, `Input.ChoiceSet`.
- Actions use types: `Action.Submit`, `Action.OpenUrl`, `Action.ShowCard`, `Action.ToggleVisibility`, `Action.Execute`.
- Prefer simple, single-purpose cards. If a flow needs multiple screens, emit multiple cards wired via `data.nextCardId` (see greentic-designer navigation convention).
- Use `wrap: true` on TextBlocks that may overflow.
- Use `separator: true` sparingly to visually group content.
- Accessibility: every Image needs `altText`. Every Action needs `title`. Input fields need `label`.
