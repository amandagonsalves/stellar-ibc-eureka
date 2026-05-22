use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AdminNotSet = 1,
    UnknownClientType = 2,
    ClientIdNotFound = 3,
    CounterpartyAlreadyRegistered = 4,
    ClientFrozen = 5,
    PortAlreadyRegistered = 6,
    InvalidIdentifierLength = 7,
    InvalidIdentifierPrefix = 8,
    InvalidIdentifierChar = 9,
    CounterpartyNotRegistered = 10,
    TimeoutAlreadyElapsed = 11,
    PortNotRegistered = 12,
    PayloadsEmpty = 13,
    ReceiptAlreadyExists = 14,
    PacketCounterpartyMismatch = 15,
    MembershipVerificationFailed = 16,
    NonMembershipVerificationFailed = 17,
    NoCommitmentForSequence = 18,
    CommitmentMismatch = 19,
    AckAlreadyExists = 20,
    NoReceiptForSequence = 21,
    TimeoutNotYetElapsed = 22,
}
