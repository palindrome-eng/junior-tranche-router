use anchor_lang::prelude::*;

#[error_code]
pub enum RlpError {
    #[msg("InvalidSigner")]
    InvalidSigner,

    #[msg("InvalidInput")]
    InvalidInput,

    #[msg("AssetNotWhitelisted")]
    AssetNotWhitelisted,

    #[msg("DepositTooLow")]
    DepositTooLow,

    #[msg("DepositCapOverflow")]
    DepositCapOverflow,

    #[msg("NotEnoughFunds")]
    NotEnoughFunds,

    #[msg("NotEnoughReceiptTokens")]
    NotEnoughReceiptTokens,

    #[msg("NotEnoughFundsToSlash")]
    NotEnoughFundsToSlash,

    #[msg("DepositsLocked")]
    DepositsLocked,

    #[msg("DepositsOpen")]
    DepositsOpen,

    #[msg("DepositsNotSlashed")]
    DepositsNotSlashed,

    #[msg("AllDepositsSlashed")]
    AllDepositsSlashed,

    #[msg("SlashAmountMismatch")]
    SlashAmountMismatch,

    #[msg("ShareConfigOverflow")]
    ShareConfigOverflow,

    #[msg("Frozen")]
    Frozen,

    #[msg("InvalidOracle")]
    InvalidOracle,

    #[msg("MathOverflow")]
    MathOverflow,

    #[msg("LockupInForce")]
    LockupInForce,

    #[msg("BoostNotApplied")]
    BoostNotApplied,

    #[msg("InvalidSigners")]
    InvalidSigners,

    #[msg("TransferSignatureRequired")]
    TransferSignatureRequired,

    #[msg("ColdWalletNotSlashed")]
    ColdWalletNotSlashed,

    #[msg("PermissionsTooLow")]
    PermissionsTooLow,

    #[msg("WithdrawalThresholdOverflow")]
    WithdrawalThresholdOverflow,

    #[msg("PoolImbalance")]
    PoolImbalance,

    #[msg("InvalidReceiptTokenSetup")]
    InvalidReceiptTokenSetup,

    #[msg("InvalidReceiptTokenDecimals")]
    InvalidReceiptTokenDecimals,

    #[msg("InvalidReceiptTokenMintAuthority")]
    InvalidReceiptTokenMintAuthority,

    #[msg("InvalidReceiptTokenSupply")]
    InvalidReceiptTokenSupply,

    #[msg("InvalidReceiptTokenFreezeAuthority")]
    InvalidReceiptTokenFreezeAuthority,

    #[msg("MinimumSuperadminsRequired")]
    MinimumSuperadminsRequired,

    #[msg("IntentValueTooLow")]
    IntentValueTooLow,

    #[msg("WithdrawalNeedsIntent")]
    WithdrawalNeedsIntent,

    #[msg("PriceError")]
    PriceError,

    #[msg("CooldownInForce")]
    CooldownInForce,

    #[msg("SlippageExceeded")]
    SlippageExceeded,

    #[msg("InvalidTokenOrder")]
    InvalidTokenOrder,

    #[msg("ActionFrozen")]
    ActionFrozen,

    #[msg("ActionNotFound")]
    ActionNotFound,

    #[msg("NoEntriesLeft")]
    NoEntriesLeft,

    #[msg("RoleNotUnderAction")]
    RoleNotUnderAction,

    #[msg("ActionHasAssignedRole")]
    ActionHasAssignedRole,

    #[msg("InvalidState")]
    InvalidState,

    #[msg("IncorrectAdmin")]
    IncorrectAdmin,

    #[msg("SameAdmin")]
    SameAdmin,

    #[msg("AlreadyFrozen")]
    AlreadyFrozen,

    #[msg("AlreadyUnfrozen")]
    AlreadyUnfrozen,

    #[msg("OracleDataTooStale")]
    OracleDataTooStale,

    #[msg("PoolAssetFrozen")]
    PoolAssetFrozen,

    #[msg("PoolAssetNotFrozen")]
    PoolAssetNotFrozen,

    #[msg("CannotRemoveLastAsset")]
    CannotRemoveLastAsset,

    #[msg("PoolHasNoProtectedVault")]
    PoolHasNoProtectedVault,

    #[msg("ProtectedVaultMismatch")]
    ProtectedVaultMismatch,

    #[msg("NoNavLossToCover")]
    NoNavLossToCover,

    #[msg("NavCoverageExceedsLoss")]
    NavCoverageExceedsLoss,

    #[msg("ProtectedVaultMintMismatch")]
    ProtectedVaultMintMismatch,

    #[msg("InvalidOracleConfiguration")]
    InvalidOracleConfiguration,

    #[msg("InvalidOracleDiscriminator")]
    InvalidOracleDiscriminator,

    #[msg("UnsupportedOracleFeed")]
    UnsupportedOracleFeed,

    #[msg("OracleProviderMismatch")]
    OracleProviderMismatch,

    #[msg("MissingOracleLeg")]
    MissingOracleLeg,

    #[msg("OracleConfidenceTooWide")]
    OracleConfidenceTooWide,

    /// Return data that the Data Streams verifier did not set. The return-data slot is one
    /// global buffer. Absent bytes and bytes from another program get the same refusal.
    #[msg("UnverifiedOracleReport")]
    UnverifiedOracleReport,

    /// A report whose `validFromTimestamp` is still in the future. `OracleDataTooStale` names
    /// the opposite direction.
    #[msg("OracleNotYetValid")]
    OracleNotYetValid,

    /// A report past its `expiresAt`. This is the bound the DON sets on verification.
    /// `OracleDataTooStale` is the bound the leg sets on the observation.
    #[msg("OracleReportExpired")]
    OracleReportExpired,

    /// A re-post whose `observationsTimestamp` is not strictly newer than the stored one.
    #[msg("OracleReportNotNewer")]
    OracleReportNotNewer,

    /// A stored exponent that disagrees with the scale the reading leg declares.
    #[msg("OracleScaleMismatch")]
    OracleScaleMismatch,

    /// A price account that the oracle program of its branch does not own.
    #[msg("InvalidOracleOwner")]
    InvalidOracleOwner,

    /// A `PriceUpdateV2` whose write authority is not the authority PDA of this program.
    /// Somebody else can re-post such an account at will.
    #[msg("InvalidOracleAuthority")]
    InvalidOracleAuthority,

    /// A `PriceUpdateV2` at `VerificationLevel::Partial`. It carries fewer signatures than the
    /// full guardian quorum of the receiver.
    #[msg("OracleNotFullyVerified")]
    OracleNotFullyVerified,

    /// A chain whose composed price sits outside the band the authority supplied to
    /// `update_oracle`.
    #[msg("ComposedPriceOutOfBand")]
    ComposedPriceOutOfBand,

    /// An asset priced on a money path whose chain has never resolved against a band. Every
    /// leg field agrees with itself. That agreement says nothing about the scale the chain
    /// composes at.
    #[msg("OracleChainNeverResolved")]
    OracleChainNeverResolved,

    /// No longer raised. A timed-out cooldown is now closed by `withdraw`. The variant stays so
    /// the codes after it do not move.
    #[msg("CooldownExpired")]
    CooldownExpired,

    /// The program returns this error when a slash has driven the pool value to zero and real LP
    /// tokens still remain. A deposit priced at the launch rate would hand most of its value to
    /// those old holders. The pool stays closed until the holders withdraw and the supply falls
    /// back to the dead shares.
    #[msg("PoolWipedOut")]
    PoolWipedOut,

    /// A withdraw while the senior vault this pool covers shows a loss `slash` could still cover
    /// from this pool. Paying out first would let the exit leave at pre-slash value and leave the
    /// loss to the LPs who stay. The withdraw succeeds once the slash lands.
    #[msg("UncoveredLoss")]
    UncoveredLoss,

    /// The signer of a `withdraw` is not the owner, and `keeper_allowed` on the cooldown is
    /// false. The payout depends on the price when `withdraw` runs. So only the owner picks that
    /// time, unless the owner set `keeper_allowed` in `request_withdrawal`.
    #[msg("KeeperNotAllowed")]
    KeeperNotAllowed,
}
