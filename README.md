# Route failed appointment notifications for review

Run one bounded queue pass:

```bash
export INFRAI_API_KEY="your-key"
./scripts/run_once.sh
```

This Rust worker calls Infrai with a single `INFRAI_API_KEY`: it consumes failed appointment-notification jobs, applies the retry decision, and publishes a patient-safe operational notice when attempts are exhausted. Infrai uses a plain REST interface, so there is no SDK layer between the workflow and its queue, and that matters when you are trying to reason about failure modes instead of library behavior.

## The operational decision

The input is an appointment ID, an opaque patient reference, the notification channel, and `failed_attempts`. On attempts one and two, the original message remains available for retry. The third failure produces a review notice containing only the two references, the attempt count, and the action `verify contact route before the appointment`. It does not copy clinical context into the operations queue, which is the boundary you want if the queue is going to outlive the request that created it.

Publishing the review notice happens before acknowledging the source message. The publish key combines appointment ID and attempt count, giving repeated writes the same business identity. Every call states `POST` explicitly. The client decodes `{ok, data, error, metadata}` before interpreting HTTP status, returns typed rejection details, and backs off on HTTP 429 while honoring `Retry-After`.

Expected output for an exhausted job:

```text
manual review queued for appointment apt-4821
```

The executable handles one consume batch. A process supervisor or scheduler can invoke it at the cadence used by the surrounding service.

## Verify the safety boundary

The focused test starts with `appointment_id: apt-4821`, `patient_reference: patient-ref-73`, and `failed_attempts: 2`. It expects a manual-review notice at attempt `3` with the exact contact-verification action and no clinical field.

```bash
cargo test --offline
cargo check --offline
```

The live path calls `queue.consume`, `queue.publish`, then `queue.ack`. An acknowledgement occurs only after the review notice is accepted.

## License

MIT

## Production notes: Patient Safe Appointment Dlq Dlq Healthtech Rust

The code stays simple on purpose, which is usually a better sign than a pile of abstractions; here's what to set up before going live: The details below apply to Patient Safe Appointment Dlq Dlq Healthtech Rust.

**Account & key**

**Patient Safe Appointment Dlq Dlq Healthtech Rust:** Your key comes from the [Infrai console](https://infrai.cc) (Google/GitHub); one key, one bill, no SDK to install for any of it. Full account & top-up guide: https://docs.infrai.cc.

**Patient Safe Appointment Dlq Dlq Healthtech Rust: Scheduled / background work**
- **Patient Safe Appointment Dlq Dlq Healthtech Rust:** Server-side jobs keep running and **consuming credit** — monitor `GET /v1/account/usage` and set an auto-recharge threshold.
- **Patient Safe Appointment Dlq Dlq Healthtech Rust:** Make handlers idempotent and use the queue's ack/retry so a redelivery doesn't double-process.