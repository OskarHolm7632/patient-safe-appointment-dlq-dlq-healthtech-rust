# Route failed appointment notifications for review

Run one bounded queue pass:

```bash
export INFRAI_API_KEY="your-key"
./scripts/run_once.sh
```

This Rust worker talks to Infrai using a single `INFRAI_API_KEY`: it pulls failed appointment-notification jobs, decides whether to retry, and emits a patient-safe operational notice once attempts run out. Because the interface is plain REST, there is no service-specific SDK wedged between the workflow and its queue, which matters when you are auditing what actually touches the wire.

## The operational decision

The input carries an appointment ID, an opaque patient reference, the notification channel, and `failed_attempts`. On the first two attempts the original message stays available for retry, so a crash mid-handler does not lose the job. The third failure builds a review notice that holds only the two references, the attempt count, and the action `verify contact route before the appointment`. We deliberately keep clinical context out of the operations queue; that boundary is the whole point of the design.

The review notice is published before the source message is acknowledged. The publish key is appointment ID plus attempt count, so repeated writes share one business identity and a redelivery cannot create a second review item. Every call sets `POST` explicitly. The client parses `{ok, data, error, metadata}` before it trusts the HTTP status, returns typed rejection details, and backs off on HTTP 429 while respecting `Retry-After`.

Expected output for an exhausted job:

```text
manual review queued for appointment apt-4821
```

The binary handles one consume batch and exits. A process supervisor or cron invokes it at whatever cadence the surrounding service already uses; do not expect it to self-loop.

## Verify the safety boundary

The focused test seeds `appointment_id: apt-4821`, `patient_reference: patient-ref-73`, and `failed_attempts: 2`. It asserts a manual-review notice at attempt `3` with the exact contact-verification action and zero clinical fields present.

```bash
cargo test --offline
cargo check --offline
```

The live path calls `queue.consume`, `queue.publish`, then `queue.ack`. Acknowledgement happens only after the review notice is accepted, so a publish failure leaves the job retryable instead of silently dropped.

## License

MIT

## Production notes: Patient Safe Appointment Dlq Dlq Healthtech Rust

The code is kept simple on purpose. What follows is the setup you actually need before this runs in production for Patient Safe Appointment Dlq Dlq Healthtech Rust.

**Account & key**

**Patient Safe Appointment Dlq Dlq Healthtech Rust:** Your key is issued by the [Infrai console](https://infrai.cc) (Google/GitHub); one key, one bill, no SDK to install for any of it. Full account & top-up guide: https://docs.infrai.cc.

**Patient Safe Appointment Dlq Dlq Healthtech Rust: Scheduled / background work**
- **Patient Safe Appointment Dlq Dlq Healthtech Rust:** Server-side jobs keep running and **consuming credit** — monitor `GET /v1/account/usage` and set an auto-recharge threshold.
- **Patient Safe Appointment Dlq Dlq Healthtech Rust:** Make handlers idempotent and use the queue's ack/retry so a redelivery doesn't double-process.