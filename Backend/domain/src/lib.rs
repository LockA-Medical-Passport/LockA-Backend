//! Core domain types, validation, and business rules: entity structs, their
//! Postgres-native enums, and the repository traits that describe how the
//! rest of the service reads and writes them.
//!
//! No I/O lives here (`storage` provides the `sqlx`-backed implementations of
//! these traits) — the one exception is `sqlx::Type`, `serde::Serialize`, and
//! `serde::Deserialize` derives on the enums and entity structs, a
//! derive-only coupling `docs/schema.md`'s "Enumerations" convention already
//! anticipates.
//!
//! Repository traits use plain `async fn`, not `#[async_trait]`: call sites
//! use concrete generic types (`impl PatientRepository`), never
//! `dyn PatientRepository`, so object-safety isn't a requirement here.

pub mod consent;
pub mod device;
pub mod error;
pub mod patient;
pub mod provider;
pub mod record;

pub use consent::{
    AccessRequest, AccessRequestChainUpdate, AccessRequestStatus, ConsentGrant,
    ConsentGrantChainUpdate, ConsentRepository, PurposeCode, RecordCategory,
};
pub use device::{
    DeviceReadingIndexEntry, DeviceRegistration, DeviceRegistrationChainUpdate, DeviceRepository,
    DeviceStatus, DeviceType, NewDeviceReadingIndexEntry,
};
pub use error::RepoError;
pub use patient::{Patient, PatientChainUpdate, PatientRepository, PatientStatus};
pub use provider::{
    NewProvider, Provider, ProviderRepository, ProviderStaff, ProviderStaffChainUpdate,
    ProviderStaffRepository, ProviderType, StaffRole, StaffStatus, VerificationStatus,
};
pub use record::{NewRecordIndexEntry, RecordIndexEntry, RecordIndexRepository, StorageBackend};

mod auth;
pub use auth::ChallengeRepository;
