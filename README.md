# LockA Backend

Backend service for authentication, encrypted record storage, provider verification, file uploads, notifications, indexing, and API endpoints.

## About LockA Medical Passport

LockA Medical Passport is a patient-controlled digital health passport for secure, private, interoperable, and verifiable medical records across Africa. Patients own and control their medical identity, decide who can access their health data, for what purpose, and for how long — similar to how a school locker gives a student a safe, personal space for important items.

This backend targets the **Stellar Soroban version** of LockA. An earlier version of the product was designed on EVM with Solidity smart contracts (see [LockA-Medical-Passport-Monorepo](https://github.com/Dannyswiss1/LockA-Medical-Passport-Monorepo) and the [live reference build](https://locka.remixdapp.eth.limo/)); that version has no backend service. This repository is the backend/API for the Stellar Soroban rebuild, and does not share code with the EVM version.

The full product documentation is maintained at [LockA-Documentation](https://github.com/LockA-Medical-Passport/LockA-Documentation/blob/main/Documentation.md).

> **Privacy rule:** No raw medical record, diagnosis, prescription, lab result, identity document, or personally identifiable medical information is stored on-chain. The chain (Stellar/Soroban) only holds identities, provider registrations, consent state, record hashes/commitments, and audit events. Encrypted records and detailed metadata live off-chain, and this backend is the layer that enforces that boundary.

## Role in the Platform

LockA is a healthcare data access network with multiple interfaces connected to a shared protocol and backend:

| Component | Purpose |
| --- | --- |
| Patient Client | Mobile/web app for patients to create a passport, manage records, approve/revoke access, and present a QR code. |
| Provider Client | Web dashboard for hospitals, clinics, labs, pharmacies, and insurers to request access and update approved records. |
| **Backend/API (this repo)** | Authentication, encrypted file handling, indexing, notifications, provider verification, Soroban contract interaction, and integrations. |
| Smart Contracts | Soroban contracts for identity registry, provider registry, consent, access permissions, record commitments, and audit events. |

```text
Patient Client
| create passport / approve consent / revoke consent
v
Backend and API Layer (this repo) <------- Provider Client
| encrypted upload/download        | request access / add records
| indexing / notifications         |
v                                   v
Encrypted Health Vault        Stellar/Soroban Contracts
| encrypted records            | patient identity registry
| files, metadata, attachments | provider registry
| AI summaries                 | consent and access control
                                | record hash commitments
                                | audit events

External Integrations
Labs, pharmacies, insurers, IoT devices, wearables, public health systems
```

## Technology Stack

| Layer | Technology / Use |
| --- | --- |
| Language | Rust |
| Blockchain | Stellar network with Soroban smart contracts for consent, provider registry, record commitments, and audit logs |
| Stellar Access | Stellar RPC for simulating/submitting contract invocations and consuming contract events; Rust Stellar/Soroban SDKs for transaction building and generated contract bindings |
| Wallet / Signing | Freighter wallet for browser-based, XLM-compatible transaction signing by patients and providers; the backend builds unsigned transactions/XDR for the client to sign via Freighter |
| Database | PostgreSQL for application metadata, provider records, access request indexes, and other non-sensitive relational data |
| Encrypted Storage | IPFS/Filecoin, S3-compatible, or other object storage with client-side or server-side encryption and strict access policies |
| Zero-Knowledge Layer | ZK proof generation/verification service (in-process or dedicated) for selective disclosure and privacy-preserving access checks |
| AI Layer | AI-assisted record summaries, missing-record detection, and privacy workflow auditing, scoped to consented records only |
| Notifications | Email, SMS, push, and/or WhatsApp/Telegram for access requests, approvals, revocations, and record updates |
| Indexing | Backend indexer consuming Stellar RPC events for consent changes and record updates |
| Testing | Unit, integration, contract-interaction, and end-to-end tests |

Note: the general LockA documentation describes a Node.js/TypeScript backend as one option for the platform. This repository instead implements the backend in Rust so it shares tooling and types with the Soroban contracts and can use the Soroban Rust SDK directly.

## Responsibilities

- **Auth & identity**: onboarding patients and providers, associating Stellar accounts with passport/provider identifiers.
- **Provider verification**: registering and verifying hospitals, clinics, labs, pharmacies, and insurers before they can request access.
- **Consent & access workflows**: relaying access requests and approvals/revocations between clients and the `ConsentAccessControl` Soroban contract, enforcing time-limited and purpose-limited access.
- **Encrypted record storage**: encrypting, storing, and serving off-chain health records and attachments; submitting record hashes/commitments to the `RecordCommitmentRegistry` contract.
- **Device/IoT verification**: validating signed readings from approved medical devices and wearables before encrypting and anchoring them off-chain, per the `DeviceAttestationRegistry` contract.
- **Chain indexing**: listening to Soroban contract events (consent grants/revocations, record updates, provider changes, audit events) and maintaining fast application-side indexes.
- **ZK verification**: verifying zero-knowledge proofs for selective disclosure (e.g., vaccination status, insurance eligibility, provider authorization) without exposing underlying records.
- **AI-assisted workflows**: generating patient summaries and flagging missing records or over-broad access requests, using only consented data; AI output is assistive, never a diagnosis.
- **Notifications**: informing patients and providers of access requests, approvals, revocations, and record updates.

## Related Smart Contracts (Soroban)

The backend interacts with the following Soroban contracts (implemented in the corresponding contracts repository):

| Contract | Responsibility |
| --- | --- |
| PatientIdentityRegistry | Patient passport identifiers, public keys, recovery configuration, identity commitments |
| ProviderRegistry | Verified hospitals, clinics, labs, pharmacies, insurers, and authorized staff accounts |
| ConsentAccessControl | Creation, approval, limitation, expiry, and revocation of provider access permissions |
| RecordCommitmentRegistry | Hashes/commitments of encrypted records, record categories, issuer references, update events |
| DeviceAttestationRegistry | Approved medical devices and IoT/wearable data sources for verifiable readings |
| AuditEventEmitter | Events for access requests, consent grants, revocations, record additions, provider updates, device attestations |

## Repository Structure (Platform-Wide)

| Repository | Scope |
| --- | --- |
| locka-patient-client | Patient onboarding, medical passport, QR sharing, consent approval, record viewing, access history |
| locka-provider-client | Provider registration, access request workflow, approved record viewing, treatment/lab/prescription updates |
| **LockA-Backend (this repo)** | Auth, provider verification, encrypted storage, ZK verifier integration, Soroban/Stellar event indexer, notifications, AI summaries |
| locka-contracts | Soroban contracts, contract tests, deployment scripts, generated client bindings, Stellar testnet configuration |

## MVP Scope (Backend-Relevant)

- Patient registration and medical passport creation
- Provider registration and verification status tracking
- Access request workflow between patients and providers
- Consent approval, limitation, expiry, and revocation, synced with `ConsentAccessControl`
- Encrypted health record upload and retrieval
- Record hash/commitment anchoring on Stellar via `RecordCommitmentRegistry`
- Basic ZK proof verification for one or two claims (e.g., vaccination proof, provider authorization proof)
- Contract event indexer for identity, provider registry, consent, and record commitment events
- Audit log accessible to the patient
- Basic AI-generated patient summary from approved records