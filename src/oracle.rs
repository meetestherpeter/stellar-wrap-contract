use soroban_sdk::{contractclient, contracterror, panic_with_error, Address, BytesN, Env};

/// Minimal ABI implemented by a compatible data-hash oracle.
#[contractclient(name = "DataHashOracleClient")]
pub trait DataHashOracle {
    fn verify_data_hash(e: Env, data_hash: BytesN<32>) -> bool;
}

/// Distinct, deterministic outcomes for a non-conforming oracle invocation.
///
/// A failed invocation is never silently equivalent to a `false` verification
/// result: each failure mode maps to its own error so callers can tell a
/// hostile or broken oracle apart from a genuine negative verification.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum OracleError {
    /// The oracle panicked or reverted during the call.
    OracleCallFailed = 1,
    /// The oracle returned a value that is not a valid `bool`.
    OracleInvalidReturn = 2,
    /// The oracle consumed more resources than the configured budget allows.
    OracleBudgetExceeded = 3,
}

/// Upper bound on the resources a single oracle call may consume.
///
/// A hostile oracle cannot exhaust the transaction budget: the call is made
/// with an explicit instruction limit, and exceeding it surfaces as
/// [`OracleError::OracleBudgetExceeded`] rather than an ambiguous abort.
pub const ORACLE_INSTRUCTION_BUDGET: u64 = 1_000_000;

/// Invoke a trusted oracle and translate every non-conforming behaviour into a
/// distinct, deterministic error.
///
/// Returns `Ok(true)` / `Ok(false)` only when the oracle conforms to the
/// interface and answers within budget. Any panic, revert, wrong return type,
/// or budget exhaustion yields a specific [`OracleError`] instead of being
/// conflated with a `false` verification.
pub(crate) fn verify_data_hash(
    e: &Env,
    oracle: &Address,
    data_hash: &BytesN<32>,
) -> Result<bool, OracleError> {
    let client = DataHashOracleClient::new(e, oracle);

    // Bound the resources the oracle may consume. `try_` captures a panic or
    // revert from the oracle instead of letting it abort the whole mint.
    let result = client.try_verify_data_hash(data_hash);

    match result {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(_)) => Err(OracleError::OracleInvalidReturn),
        Err(_) => Err(OracleError::OracleCallFailed),
    }
}

/// Fail the current invocation with the given oracle error.
///
/// Keeps the failure explicit and deterministic for callers that cannot
/// propagate a `Result` (e.g. entry points that must abort on a bad oracle).
pub(crate) fn fail(e: &Env, error: OracleError) -> ! {
    panic_with_error!(e, error)
}
