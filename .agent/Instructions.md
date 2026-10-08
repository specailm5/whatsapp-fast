---
name: scalable-maintainable-project-architecture
description: Establish scalable, maintainable, testable, and replaceable software architecture for new projects. MUST be followed during project scaffolding and initial architecture decisions. Use when creating a new application, repository, service, API, CLI, or substantial software project.
---

# Scalable & Maintainable Project Architecture

## CRITICAL: Apply This Skill During Scaffolding

**This skill MUST be applied before scaffolding any new project.**

Do not start by generating files, folders, models, routes, services, or infrastructure until the architectural principles below have been considered.

The goal is not to over-engineer the project. The goal is to create a **simple starting architecture that can grow without requiring a rewrite**.

Before creating a new project:

1. Identify the major domains/features.
2. Define clear responsibilities and boundaries.
3. Decide where business logic belongs.
4. Separate infrastructure concerns from business logic.
5. Establish testing and configuration conventions.
6. Define how external integrations will be isolated.
7. Consider how the application can scale horizontally.
8. Establish logging, error handling, migrations, and deployment conventions.
9. Prefer a modular monolith initially unless there is a concrete reason to use distributed services.
10. Document important architectural decisions.

**Treat these decisions as part of scaffolding, not as cleanup work to do later.**

A poorly structured prototype often becomes the architecture of the production system. Prevent that at the beginning.

---

# Core Principles

## 1. Separation of Responsibilities

Each component should have one clear responsibility.

Prefer:

```text
API Controller
    ↓
Application Service
    ↓
Repository
    ↓
Database
```

Avoid putting authentication, validation, business rules, database operations, notifications, and external API calls into one function.

### Bad

```python
def create_order(request):
    user = authenticate(request)
    validate_request(request)

    order = db.insert_order(request)

    stripe.charge(user, order.total)

    send_email(user.email)

    analytics.record("order_created")

    return order
```

### Better

```python
order = order_service.create_order(data)

payment_service.charge(order)
notification_service.notify_order_created(order)
analytics_service.record_order_created(order)

return order
```

Each service has a focused responsibility.

---

# 2. Organize Code Around Features / Domains

For larger applications, prefer feature-oriented organization over dumping every file type into global folders.

### Prefer

```text
src/
├── users/
│   ├── controller.py
│   ├── service.py
│   ├── repository.py
│   ├── models.py
│   └── tests/
│
├── orders/
│   ├── controller.py
│   ├── service.py
│   ├── repository.py
│   ├── models.py
│   └── tests/
│
├── authentication/
│   ├── service.py
│   └── tests/
│
└── notifications/
    ├── service.py
    ├── providers/
    └── tests/
```

### Avoid

```text
controllers/
models/
services/
repositories/
utils/
helpers/
```

with hundreds of unrelated files mixed together.

Feature boundaries make large codebases easier to navigate and maintain.

---

# 3. Keep Business Logic Independent of Infrastructure

Business rules should not depend directly on a particular database, cloud provider, messaging provider, or third-party API.

Prefer:

```text
Business Logic
      ↓
Interface / Contract
      ↓
Infrastructure Implementation
```

Example:

```python
class PaymentProvider:
    def charge(self, amount):
        raise NotImplementedError
```

Implementations:

```python
class StripePaymentProvider(PaymentProvider):
    ...

class MockPaymentProvider(PaymentProvider):
    ...
```

The order system depends on the `PaymentProvider` contract rather than directly on Stripe.

This makes integrations replaceable and testing easier.

---

# 4. Avoid Vendor Lock-In Where Practical

Do not spread provider-specific code throughout the application.

### Bad

```python
stripe.Customer.create(...)
stripe.PaymentIntent.create(...)
```

in dozens of business-logic files.

### Better

```python
payment_service.charge(customer, amount)
```

with the provider implementation isolated:

```text
integrations/
└── payments/
    ├── interface.py
    ├── stripe.py
    └── mock.py
```

If the provider changes, the rest of the application should require minimal modification.

---

# 5. Do Not Duplicate Business Logic

If the same rule appears in multiple places, centralize it.

### Bad

```python
# route A
if user.role == "admin":
    ...

# route B
if user.role == "admin":
    ...

# route C
if user.role == "admin":
    ...
```

### Better

```python
permission_service.can_manage_users(user)
```

The rule exists in one authoritative location.

This prevents inconsistent behavior as the application grows.

---

# 6. Prefer Explicit Contracts

Components should communicate through clear inputs and outputs.

Use:

- typed interfaces
- schemas
- DTOs where appropriate
- validation
- explicit return types
- documented API contracts

Avoid functions that accept arbitrary dictionaries with undocumented fields throughout the application.

### Better

```python
class CreateUserRequest:
    email: str
    name: str
```

rather than:

```python
create_user(data: dict)
```

with unknown expectations.

---

# 7. Start With a Modular Monolith

Do **not** automatically create microservices.

For most new applications, start with a modular monolith:

```text
                    API
                     │
              ┌──────┴──────┐
              │ Application  │
              │   Modules    │
              ├──────────────┤
              │ Users        │
              │ Orders       │
              │ Payments     │
              │ Notifications│
              └──────┬───────┘
                     │
                  Database
```

Maintain clear internal boundaries.

Extract a module into a separate service only when there is a concrete reason, such as:

- independent scaling requirements
- independent deployment requirements
- isolation requirements
- team ownership boundaries
- significantly different infrastructure requirements

Microservices introduce operational complexity. Do not use them merely because they sound scalable.

---

# 8. Design for Horizontal Scaling

Application servers should preferably be stateless.

Good:

```text
                 Load Balancer
                /      |      \
               /       |       \
          Server 1  Server 2  Server 3
               \       |       /
                \      |      /
                 Shared Services
```

Avoid storing important state only in local process memory or local disk.

Bad:

```python
active_users = {}
```

if another application server needs access to that state.

Use shared infrastructure where appropriate:

```text
PostgreSQL → persistent application data
Redis      → cache / ephemeral shared state
Object     → files and large assets
Queue      → asynchronous work
```

---

# 9. Use Background Jobs for Expensive Work

Do not make users wait for work that can happen asynchronously.

### Bad

```text
HTTP Request
    ↓
Process image
    ↓
Call external API
    ↓
Send 10 emails
    ↓
Generate report
    ↓
HTTP Response
```

### Better

```text
HTTP Request
    ↓
Save request
    ↓
Queue job
    ↓
HTTP Response
          \
           ↓
         Worker
           ├── Process image
           ├── Call external API
           ├── Send notifications
           └── Generate report
```

This allows worker capacity to scale independently.

---

# 10. Separate Transactional Storage From Search

The primary database should remain the source of truth.

For applications requiring substantial search:

```text
PostgreSQL
    │
    └── Source of truth
          │
          ▼
      Search Index
```

The search index should be rebuildable from authoritative data.

Never make a cache or search index the only copy of important data.

---

# 11. Design External Integrations as Adapters

External systems change.

Create a provider interface:

```python
class PublicRecordProvider:
    def fetch_records(self):
        raise NotImplementedError
```

Then implement providers:

```text
integrations/
└── public_records/
    ├── interface.py
    ├── provider_a.py
    ├── provider_b.py
    └── provider_c.py
```

The application consumes a normalized internal representation:

```text
External Source
      ↓
Adapter
      ↓
Normalized Data
      ↓
Application
```

If an external API changes, the adapter changes—not the entire application.

---

# 12. Use API Versioning

Public or mobile APIs should have a compatibility strategy.

Example:

```text
/api/v1/users
/api/v1/orders
/api/v1/search
```

When breaking changes are required:

```text
/api/v2/search
```

Do not casually break clients that may still be running an older application version.

---

# 13. Treat Errors and Failures as Normal

Distributed systems and external dependencies fail.

Account for:

- timeouts
- retries
- rate limits
- unavailable services
- malformed responses
- duplicate requests
- partial failures
- expired authentication
- database failures

Use appropriate mechanisms:

```text
Timeouts
Retries
Exponential backoff
Circuit breakers where appropriate
Idempotency
Dead-letter queues
Structured errors
```

Do not retry blindly. Retrying non-idempotent operations can create duplicate actions.

---

# 14. Make Important Operations Idempotent

A request may be sent twice because of:

- network retries
- mobile reconnects
- user double-clicks
- load balancer retries
- client bugs

Example:

```text
POST /payments
Idempotency-Key: abc123
```

The backend should recognize that `abc123` has already been processed.

This prevents:

```text
One request
    ↓
Two charges
```

---

# 15. Use Database Constraints, Not Just Application Validation

Application validation is useful, but the database should also protect critical invariants.

Example:

```text
Application:
"Email looks valid."

Database:
UNIQUE(email)
```

Use:

- foreign keys
- unique constraints
- indexes
- check constraints
- transactions

Do not assume every caller will go through the same application validation.

---

# 16. Use Database Migrations

Never manually modify production databases as a normal workflow.

Use versioned migrations:

```text
migrations/
├── 001_initial_schema
├── 002_add_users
├── 003_add_orders
└── 004_add_indexes
```

The database schema becomes reproducible across:

```text
Development
Testing
Staging
Production
```

---

# 17. Testing Strategy

Use multiple levels of tests.

```text
tests/
├── unit/
├── integration/
├── API/
├── end_to_end/
└── security/
```

### Unit tests

Test isolated business logic.

### Integration tests

Test components working together.

### API tests

Test HTTP contracts.

### End-to-end tests

Test important user workflows.

### Security tests

Test authentication, authorization, input handling, and other security-critical behavior.

Do not chase 100% coverage blindly. Prioritize critical business behavior and failure-prone areas.

---

# 18. Logging Must Be Structured

Avoid relying on:

```python
print("something went wrong")
```

Prefer structured logs:

```json
{
  "event": "order_created",
  "order_id": "12345",
  "user_id": "789",
  "timestamp": "2026-09-08T12:00:00Z"
}
```

Include identifiers that allow an operation to be traced across services.

Never log secrets, passwords, tokens, or unnecessary sensitive information.

---

# 19. Observability

A production system should allow developers to answer:

```text
Is the system healthy?
What is failing?
Where is it failing?
How often is it failing?
Which users/requests are affected?
```

Use appropriate:

```text
Logs
Metrics
Tracing
Health checks
Alerts
```

For distributed systems, propagate a request/correlation ID:

```text
Request ID: 8f31...

API
 ↓
Service
 ↓
Queue
 ↓
Worker
 ↓
External API
```

This makes debugging dramatically easier.

---

# 20. Configuration Must Be Externalized

Do not hard-code environment-specific values.

Bad:

```python
DATABASE_URL = "postgres://production..."
```

Better:

```python
DATABASE_URL = os.environ["DATABASE_URL"]
```

Separate:

```text
Development
Staging
Production
```

Never commit secrets to source control.

---

# 21. Security Is an Architectural Concern

Security should not be added at the end.

During scaffolding, establish:

```text
Authentication
Authorization
Input validation
Secrets management
Encryption
Rate limiting
Audit logging
Dependency management
Secure headers
```

Apply least privilege:

```text
User → only user permissions
Moderator → moderation permissions
Admin → administrative permissions
Database → minimum required permissions
```

Do not assume authentication means authorization.

---

# 22. Keep Dependencies Under Control

Every dependency adds:

```text
Maintenance
Security risk
Upgrade cost
Potential incompatibilities
```

Before adding a library, ask:

> Does this solve enough complexity to justify becoming part of the project?

Prefer mature, well-maintained dependencies when a dependency is justified.

Regularly update and audit dependencies.

---

# 23. Don't Create a Giant `utils` Folder

A giant:

```text
utils/
```

usually becomes a dumping ground.

Instead of:

```python
utils.py
```

prefer meaningful ownership:

```text
authentication/passwords.py
billing/currency.py
notifications/templates.py
search/normalization.py
```

Code should live near the domain that owns it.

---

# 24. Avoid Premature Optimization

Do not build:

```text
Kafka
Kubernetes
20 microservices
distributed caching
multi-region databases
```

because "we might have millions of users."

First create a clean architecture.

Then measure.

Then optimize the actual bottleneck.

The preferred progression is:

```text
Simple
   ↓
Measure
   ↓
Identify bottleneck
   ↓
Optimize
   ↓
Measure again
```

not:

```text
Guess bottleneck
   ↓
Build enormous infrastructure
   ↓
Discover the problem was elsewhere
```

---

# 25. Documentation Is Part of Maintainability

Document decisions that future developers would otherwise have to rediscover.

Recommended:

```text
README.md
ARCHITECTURE.md
CONTRIBUTING.md
SECURITY.md
docs/
└── decisions/
    ├── 001-database-choice.md
    ├── 002-authentication.md
    └── 003-queue-architecture.md
```

Document **why**, not just **what**.

Bad:

```text
We use PostgreSQL.
```

Better:

```text
We use PostgreSQL because the application's core data is relational,
requires transactions and constraints, and currently does not require
a distributed database.
```

---

# 26. Code Quality Rules

Prefer:

- small functions
- meaningful names
- explicit dependencies
- type safety where supported
- consistent formatting
- linting
- static analysis
- automated tests
- clear module boundaries

Avoid:

- magic values
- hidden global state
- giant classes
- giant functions
- duplicated business rules
- circular dependencies
- unexplained abstractions
- unnecessary cleverness

Readable code is a scalability feature because developer time becomes a bottleneck as the project grows.

---

# 27. Dependency Direction

A useful rule:

```text
                Infrastructure
                      ↑
                      │
API ───────→ Application ───────→ Domain
                      │
                      ↓
                  Interfaces
```

Core business logic should not depend directly on infrastructure implementations.

Prefer dependencies pointing **toward stable business concepts** rather than volatile external technologies.

---

# 28. Keep the Source of Truth Clear

For every important piece of data, know:

> Where is the authoritative source?

Example:

```text
PostgreSQL
   ↓
Source of truth

Redis
   ↓
Cache

Search index
   ↓
Derived representation

Analytics warehouse
   ↓
Derived reporting data
```

If the search index disappears, you should be able to rebuild it.

If Redis disappears, the application should recover.

This principle prevents architectural confusion.

---

# 29. Scalability Is More Than Handling Users

Consider different dimensions:

```text
Traffic
Data volume
Database size
Background jobs
File storage
External API limits
Concurrent users
Geographic distribution
Development team size
Deployment frequency
```

A system can scale technically while becoming impossible for developers to maintain.

Therefore optimize for both:

```text
Runtime scalability
+
Organizational/developer scalability
```

---

# 30. Recommended Initial Project Checklist

Before scaffolding:

```text
[ ] Identify major domains/features
[ ] Define module boundaries
[ ] Define responsibility of each module
[ ] Decide where business logic lives
[ ] Define persistence boundaries
[ ] Define external integration boundaries
[ ] Define authentication/authorization approach
[ ] Define error-handling strategy
[ ] Define logging strategy
[ ] Define testing strategy
[ ] Define configuration/secrets strategy
[ ] Define migration strategy
[ ] Define API versioning strategy
[ ] Define background-job strategy
[ ] Define deployment strategy
[ ] Define observability requirements
[ ] Document important architectural decisions
```

---

# Default Architecture Recommendation

Unless the project has unusual requirements, start here:

```text
                         Client
                           │
                           ▼
                      API / Web
                           │
                           ▼
                  ┌─────────────────┐
                  │ Modular Backend │
                  │                 │
                  │ Authentication  │
                  │ Users           │
                  │ Domain Modules  │
                  │ Services        │
                  │ Integrations    │
                  └────────┬────────┘
                           │
             ┌─────────────┼─────────────┐
             ▼             ▼             ▼
        PostgreSQL       Redis         Queue
        Source of        Cache         Jobs
         Truth                         │
                                       ▼
                                    Workers
                                       │
                            External Integrations
```

Start with this architecture **only to the extent the project actually needs each component**. Do not introduce Redis or a queue without a reason.

---

# Scaffolding Rule

When asked to create a new project, the implementation agent should **first reason about architecture and boundaries, then scaffold**.

The expected workflow is:

```text
Requirements
     ↓
Identify domains
     ↓
Define boundaries
     ↓
Choose architecture
     ↓
Define contracts
     ↓
Define data model
     ↓
Define testing strategy
     ↓
Scaffold project
     ↓
Implement features
     ↓
Test
     ↓
Review architecture
```

Not:

```text
"Create an app"
     ↓
Generate 100 files
     ↓
Figure out architecture later
```

## Final Principle

The primary architectural objective is:

> **Make every important part easy to understand, easy to test, and easy to replace.**

Good architecture should allow the project to grow from:

```text
10 users
```

to:

```text
10,000 users
```

to:

```text
1,000,000 users
```

without requiring the entire codebase to be rewritten.

At the same time, it should allow a new developer to understand a feature without understanding the entire system.

**Scalability should emerge from good boundaries, not from unnecessary complexity.**
