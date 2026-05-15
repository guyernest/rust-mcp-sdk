# Canonical Query Patterns

## Single user with their last 5 orders
```graphql
query {
  user(id: "123") {
    name
    orders(limit: 5) { id total status }
  }
}
```

## All users with at least one shipped order
```graphql
query {
  users(limit: 50) {
    id
    name
    orders { status }
  }
}
```
Filter client-side: keep users where `orders.some(o => o.status == 'SHIPPED')`.

## Avoid
- Unbounded `users` queries (always pass `limit`).
- Nested `orders` without a `limit` argument.
