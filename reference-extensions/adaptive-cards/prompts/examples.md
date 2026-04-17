# Adaptive Cards — Common Patterns

## Minimal hello card

```json
{
  "type": "AdaptiveCard",
  "version": "1.6",
  "body": [
    { "type": "TextBlock", "text": "Hello", "weight": "Bolder", "size": "Large", "wrap": true }
  ]
}
```

## Form with submit

```json
{
  "type": "AdaptiveCard",
  "version": "1.6",
  "body": [
    { "type": "Input.Text", "id": "name", "label": "Your name", "isRequired": true },
    { "type": "Input.Text", "id": "email", "label": "Email", "style": "Email" }
  ],
  "actions": [
    { "type": "Action.Submit", "title": "Continue", "data": { "nextCardId": "confirm" } }
  ]
}
```

## Branching via action data

Use `Action.Submit` with `data.nextCardId` to route between cards. Each card stands alone; routing is client-side.
