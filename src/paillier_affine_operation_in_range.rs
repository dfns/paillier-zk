//! ZK-proof of paillier operation with group commitment in range. Called Пaff-g
//! or Raff-g in the CGGMP21 paper.
//!
//! ## Description
//!
//! A party P performs a paillier affine operation with C, Y, and X
//! obtaining `D = C*X + Y`. `X` and `Y` are encrypted values of `x` and `y`. P
//! then wants to prove that `y` and `x` are at most `L` and `L'` bits,
//! correspondingly, and P doesn't want to disclose none of the plaintexts
//!
//! Given:
//! - `key0`, `pkey0`, `key1`, `pkey1` - pairs of public and private keys in
//!   paillier cryptosystem
//! - `nonce_c`, `nonce_y`, `nonce` - nonces in paillier encryption
//! - `c`, `x`, `y` - some numbers
//! - `q`, `g` such that `<g> = Zq*` - prime order group
//! - `C = key0.encrypt(c, nonce_c)`
//! - `Y' = key0.encrypt(y, nonce)`
//! - `Y = key1.encrypt(y, nonce_y)`
//! - `X = g * x`
//! - `D = key0.affine_operation!{ X * C + Y' }`, i.e.
//!   `pkey0.decrypt(D) = x * c + y`
//!
//! Prove:
//! - `bitsize(y) <= L`
//! - `bitsize(x) <= L'`
//!
//! Disclosing only: `g`, `q`, `key0`, `key1`, `C`, `D`, `Y`, `X`
//!
//! ## Example
//!
//! ``` no_run
//! # use paillier_zk::unknown_order::BigNumber;
//! use paillier_zk::paillier_affine_operation_in_range as p;
//! use paillier_zk::{L, EPSILON};
//!
//! // 0. Setup: prover and verifier share common Ring-Pedersen parameters:
//!
//! let p = BigNumber::prime(L + EPSILON + 1);
//! let q = BigNumber::prime(L + EPSILON + 1);
//! let rsa_modulo = p * q;
//! let s: BigNumber = 123.into();
//! let t: BigNumber = 321.into();
//! assert_eq!(s.gcd(&rsa_modulo), 1.into());
//! assert_eq!(t.gcd(&rsa_modulo), 1.into());
//!
//! let aux = p::Aux { s, t, rsa_modulo };
//!
//! // 1. Setup: prover prepares the paillier keys
//!
//! // this key is used to decrypt C and D, and also Y in the affine operation
//! let private_key0 = libpaillier::DecryptionKey::random().unwrap();
//! let key0 = libpaillier::EncryptionKey::from(&private_key0);
//! // this key is used to decrypt Y in this ZK-protocol
//! let private_key1 = libpaillier::DecryptionKey::random().unwrap();
//! let key1 = libpaillier::EncryptionKey::from(&private_key1);
//!
//! // 2. Setup: prover prepares the group used to encrypt x
//!
//! let q = BigNumber::from(1_000_000_007);
//! let g = BigNumber::from(2);
//! assert_eq!(g.gcd(&q), 1.into());
//!
//! // 3. Setup: prover prepares all plain texts
//!
//! // c in paper
//! let plaintext_orig = BigNumber::from(100);
//! // x in paper
//! let plaintext_mult = BigNumber::from(2);
//! // y in paper
//! let plaintext_add = BigNumber::from(28);
//!
//! // 4. Setup: prover encrypts everything on correct keys and remembers some nonces
//!
//! // C in paper
//! let (ciphertext_orig, _) = key0.encrypt(plaintext_orig.to_bytes(), None).unwrap();
//! // X in paper
//! let ciphertext_mult = g.modpow(&plaintext_mult, &q);
//! // Y' in further docs, and ρy in paper
//! let (ciphertext_add, nonce_y) = key1.encrypt(plaintext_add.to_bytes(), None).unwrap();
//! // Y and ρ in paper
//! let (ciphertext_add_action, nonce) = key0.encrypt(plaintext_add.to_bytes(), None).unwrap();
//! // D in paper
//! let transformed = key0
//!     .add(
//!         &key0.mul(&ciphertext_orig, &plaintext_mult).unwrap(),
//!         &ciphertext_add_action,
//!     )
//!     .unwrap();
//!
//! // 5. Prover computes a non-interactive proof that plaintext_add and
//! //    plaintext_mult are at most L and L' bits
//!
//! let rng = rand_core::OsRng::default();
//! let data = p::Data {
//!     g,
//!     q,
//!     key0,
//!     key1,
//!     c: ciphertext_orig,
//!     d: transformed,
//!     y: ciphertext_add,
//!     x: ciphertext_mult,
//! };
//! let pdata = p::PrivateData {
//!     x: plaintext_mult,
//!     y: plaintext_add,
//!     nonce,
//!     nonce_y,
//! };
//! let (commitment, challenge, proof) =
//!     p::compute_proof(&aux, &data, &pdata, rng);
//!
//! // 6. Prover sends this data to verifier
//!
//! # fn send(_: &p::Data, _: &p::Commitment, _: &p::Challenge, _: &p::Proof) { todo!() }
//! # fn recv() -> (p::Data, p::Commitment, p::Challenge, p::Proof) { todo!() }
//! send(&data, &commitment, &challenge, &proof);
//!
//! // 7. Verifier receives the data and the proof and verifies it
//!
//! let (data, commitment, challenge, proof) = recv();
//! let r = p::verify(&aux, &data, &commitment, &challenge, &proof);
//! ```
//!
//! If the verification succeeded, verifier can continue communication with prover

use crate::unknown_order::BigNumber;
use libpaillier::{Ciphertext, EncryptionKey, Nonce};
use rand_core::RngCore;

use crate::common::{combine, gen_inversible};
use crate::{EPSILON, L};

/// Public data that both parties know
pub struct Data {
    /// Group generator
    pub g: BigNumber,
    /// Group rank
    pub q: BigNumber,
    /// N0 in paper, public key that C was encrypted on
    pub key0: EncryptionKey,
    /// N1 in paper, public key that y -> Y was encrypted on
    pub key1: EncryptionKey,
    /// C or C0 in paper, some data encrypted on N0
    pub c: Ciphertext,
    /// D or C in paper, result of affine transformation of C0 with x and y
    pub d: BigNumber,
    /// Y in paper, y encrypted on N1
    pub y: Ciphertext,
    /// X in paper, obtained as g^x
    pub x: BigNumber,
}

/// Private data of prover
pub struct PrivateData {
    /// x or epsilon in paper, preimage of X
    pub x: BigNumber,
    /// y or delta in paper, preimage of Y
    pub y: BigNumber,
    /// rho in paper, nonce in encryption of y for additive action
    pub nonce: Nonce,
    /// rho_y in paper, nonce in encryption of y to obtain Y
    pub nonce_y: Nonce,
}

// As described in cggmp21 at page 35
/// Prover's first message, obtained by `commit`
pub struct Commitment {
    a: BigNumber,
    b_x: BigNumber,
    b_y: BigNumber,
    e: BigNumber,
    s: BigNumber,
    f: BigNumber,
    t: BigNumber,
}

/// Prover's data accompanying the commitment. Kept as state between rounds in
/// the interactive protocol.
pub struct PrivateCommitment {
    alpha: BigNumber,
    beta: BigNumber,
    r: BigNumber,
    r_y: BigNumber,
    gamma: BigNumber,
    m: BigNumber,
    delta: BigNumber,
    mu: BigNumber,
}

/// Verifier's challenge to prover. Can be obtained deterministically by
/// `challenge`
pub type Challenge = BigNumber;

/// The ZK proof. Computed by `prove`
pub struct Proof {
    z1: BigNumber,
    z2: BigNumber,
    z3: BigNumber,
    z4: BigNumber,
    w: BigNumber,
    w_y: BigNumber,
}

/// Auxiliary data known to both prover and verifier
pub struct Aux {
    /// ring-pedersen parameter
    pub s: BigNumber,
    /// ring-pedersen parameter
    pub t: BigNumber,
    /// N^ in paper
    pub rsa_modulo: BigNumber,
}

/// Create random commitment
pub fn commit<R: RngCore>(
    aux: &Aux,
    data: &Data,
    pdata: &PrivateData,
    mut rng: R,
) -> (Commitment, PrivateCommitment) {
    let two_to_l = BigNumber::one() << L;
    let two_to_l_e = BigNumber::one() << (L + EPSILON);
    let modulo_l = two_to_l * &aux.rsa_modulo;
    let modulo_l_e = &two_to_l_e * &aux.rsa_modulo;

    let alpha = BigNumber::from_rng(&two_to_l_e, &mut rng);
    let beta = BigNumber::from_rng(&two_to_l_e, &mut rng); // XXX l'
    let r = gen_inversible(data.key0.n(), &mut rng);
    let r_y = gen_inversible(data.key1.n(), &mut rng);
    let gamma = BigNumber::from_rng(&modulo_l_e, &mut rng);
    let m = BigNumber::from_rng(&modulo_l, &mut rng);
    let delta = BigNumber::from_rng(&modulo_l_e, &mut rng);
    let mu = BigNumber::from_rng(&modulo_l, &mut rng);

    let a_add = data
        .key0
        .encrypt(beta.to_bytes(), Some(r.clone()))
        .unwrap()
        .0;
    let c_to_alpha = data.key0.mul(&data.c, &alpha).unwrap();
    let a = data.key0.add(&c_to_alpha, &a_add).unwrap();
    let commitment = Commitment {
        a,
        b_x: data.g.modpow(&alpha, &data.q),
        b_y: data
            .key1
            .encrypt(beta.to_bytes(), Some(r_y.clone()))
            .unwrap()
            .0,
        e: combine(&aux.s, &alpha, &aux.t, &gamma, &aux.rsa_modulo),
        s: combine(&aux.s, &pdata.x, &aux.t, &m, &aux.rsa_modulo),
        f: combine(&aux.s, &beta, &aux.t, &delta, &aux.rsa_modulo),
        t: combine(&aux.s, &pdata.y, &aux.t, &mu, &aux.rsa_modulo),
    };
    let private_commitment = PrivateCommitment {
        alpha,
        beta,
        r,
        r_y,
        gamma,
        m,
        delta,
        mu,
    };
    (commitment, private_commitment)
}

/// Compute proof for given data and prior protocol values
pub fn prove(
    data: &Data,
    pdata: &PrivateData,
    pcomm: &PrivateCommitment,
    challenge: &Challenge,
) -> Proof {
    Proof {
        z1: &pcomm.alpha + challenge * &pdata.x,
        z2: &pcomm.beta + challenge * &pdata.y,
        z3: &pcomm.gamma + challenge * &pcomm.m,
        z4: &pcomm.delta + challenge * &pcomm.mu,
        w: combine(
            &pcomm.r,
            &BigNumber::one(),
            &pdata.nonce,
            challenge,
            data.key0.n(),
        ),
        w_y: combine(
            &pcomm.r_y,
            &BigNumber::one(),
            &pdata.nonce_y,
            challenge,
            data.key1.n(),
        ),
    }
}

/// Verify the proof
pub fn verify(
    aux: &Aux,
    data: &Data,
    commitment: &Commitment,
    challenge: &Challenge,
    proof: &Proof,
) -> Result<(), &'static str> {
    let one = BigNumber::one();
    fn fail_if(msg: &'static str, b: bool) -> Result<(), &'static str> {
        if b {
            Ok(())
        } else {
            Err(msg)
        }
    }
    // Five equality checks and two range checks
    {
        let enc = data
            .key0
            .encrypt(proof.z2.to_bytes(), Some(proof.w.clone()))
            .unwrap()
            .0;
        let lhs = data
            .key0
            .add(&data.key0.mul(&data.c, &proof.z1).unwrap(), &enc)
            .unwrap();
        let rhs = combine(&commitment.a, &one, &data.d, challenge, data.key0.nn());
        fail_if("check1", lhs == rhs)?;
    }
    {
        let lhs = data.g.modpow(&proof.z1, &data.q);
        let rhs = combine(&commitment.b_x, &one, &data.x, challenge, &data.q);
        fail_if("check2", lhs == rhs)?;
    }
    {
        let lhs = data
            .key1
            .encrypt(proof.z2.to_bytes(), Some(proof.w_y.clone()))
            .unwrap()
            .0;
        let rhs = combine(&commitment.b_y, &one, &data.y, challenge, data.key1.nn());
        fail_if("check3", lhs == rhs)?;
    }
    fail_if(
        "check4",
        combine(&aux.s, &proof.z1, &aux.t, &proof.z3, &aux.rsa_modulo)
            == combine(
                &commitment.e,
                &one,
                &commitment.s,
                challenge,
                &aux.rsa_modulo,
            ),
    )?;
    fail_if(
        "check5",
        combine(&aux.s, &proof.z2, &aux.t, &proof.z4, &aux.rsa_modulo)
            == combine(
                &commitment.f,
                &one,
                &commitment.t,
                challenge,
                &aux.rsa_modulo,
            ),
    )?;
    fail_if("range check6", proof.z1 <= &one << (L + EPSILON))?;
    fail_if(
        "range check7",
        proof.z2 <= &one << (L + EPSILON), // TODO: L'
    )?;
    Ok(())
}

/// Deterministically compute challenge based on prior known values in protocol
pub fn challenge(aux: &Aux, data: &Data, commitment: &Commitment) -> Challenge {
    use sha2::Digest;
    let mut digest = sha2::Sha512::new();

    digest.update(aux.s.to_bytes());
    digest.update(aux.t.to_bytes());
    digest.update(aux.rsa_modulo.to_bytes());

    digest.update(data.g.to_bytes());
    digest.update(data.q.to_bytes());
    digest.update(data.key0.to_bytes());
    digest.update(data.key1.to_bytes());
    digest.update(data.c.to_bytes());
    digest.update(data.d.to_bytes());
    digest.update(data.y.to_bytes());
    digest.update(data.x.to_bytes());

    digest.update(commitment.a.to_bytes());
    digest.update(commitment.b_x.to_bytes());
    digest.update(commitment.b_y.to_bytes());
    digest.update(commitment.e.to_bytes());
    digest.update(commitment.s.to_bytes());
    digest.update(commitment.f.to_bytes());
    digest.update(commitment.t.to_bytes());

    BigNumber::from_slice(digest.finalize())
}

/// Compute proof for the given data, producing random commitment and
/// deriving determenistic challenge.
///
/// Obtained from the above interactive proof via Fiat-Shamir heuristic.
pub fn compute_proof<R: RngCore>(
    aux: &Aux,
    data: &Data,
    pdata: &PrivateData,
    rng: R,
) -> (Commitment, Challenge, Proof) {
    let (comm, pcomm) = commit(aux, data, pdata, rng);
    let challenge = challenge(aux, data, &comm);
    let proof = prove(data, pdata, &pcomm, &challenge);
    (comm, challenge, proof)
}

#[cfg(test)]
mod test {
    use crate::unknown_order::BigNumber;

    use crate::{EPSILON, L};

    #[test]
    fn passing() {
        let private_key0 = libpaillier::DecryptionKey::random().unwrap();
        let key0 = libpaillier::EncryptionKey::from(&private_key0);
        let private_key1 = libpaillier::DecryptionKey::random().unwrap();
        let key1 = libpaillier::EncryptionKey::from(&private_key1);
        let plaintext: BigNumber = 228.into();
        let plaintext_orig = BigNumber::from(100);
        let plaintext_mult = BigNumber::from(2);
        let plaintext_add = BigNumber::from(28);
        let q = BigNumber::from(1_000_000_007);
        let g = BigNumber::from(2);
        // verify that g is generator in Z/q
        assert_eq!(g.gcd(&q), 1.into());
        let (ciphertext, _) = key0.encrypt(plaintext.to_bytes(), None).unwrap();
        let (ciphertext_orig, _) = key0.encrypt(plaintext_orig.to_bytes(), None).unwrap();
        let ciphertext_mult = g.modpow(&plaintext_mult, &q);
        let (ciphertext_add, nonce_y) = key1.encrypt(plaintext_add.to_bytes(), None).unwrap();
        let (ciphertext_add_action, nonce) = key0.encrypt(plaintext_add.to_bytes(), None).unwrap();
        // verify that D is obtained from affine transformation of C
        let transformed = key0
            .add(
                &key0.mul(&ciphertext_orig, &plaintext_mult).unwrap(),
                &ciphertext_add_action,
            )
            .unwrap();
        assert_eq!(
            private_key0.decrypt(&transformed).unwrap(),
            private_key0.decrypt(&ciphertext).unwrap(),
        );
        let data = super::Data {
            g,
            q,
            key0,
            key1,
            c: ciphertext_orig,
            d: transformed,
            y: ciphertext_add,
            x: ciphertext_mult,
        };
        let pdata = super::PrivateData {
            x: plaintext_mult,
            y: plaintext_add,
            nonce,
            nonce_y,
        };

        let p = BigNumber::prime(L + EPSILON + 1);
        let q = BigNumber::prime(L + EPSILON + 1);
        let rsa_modulo = p * q;
        let s: BigNumber = 123.into();
        let t: BigNumber = 321.into();
        assert_eq!(s.gcd(&rsa_modulo), 1.into());
        assert_eq!(t.gcd(&rsa_modulo), 1.into());
        let aux = super::Aux { s, t, rsa_modulo };

        let (commitment, challenge, proof) =
            super::compute_proof(&aux, &data, &pdata, rand_core::OsRng::default());
        let r = super::verify(&aux, &data, &commitment, &challenge, &proof);
        match r {
            Ok(()) => (),
            Err(e) => panic!("{}", e),
        }
    }

    #[test]
    fn failing() {
        let private_key0 = libpaillier::DecryptionKey::random().unwrap();
        let key0 = libpaillier::EncryptionKey::from(&private_key0);
        let private_key1 = libpaillier::DecryptionKey::random().unwrap();
        let key1 = libpaillier::EncryptionKey::from(&private_key1);
        let plaintext_orig = BigNumber::from(1337);
        let plaintext_mult = BigNumber::one() << (L + EPSILON) + 1;
        let plaintext_add = BigNumber::one() << (L + EPSILON) + 2;
        let q = BigNumber::from(1_000_000_007);
        let g = BigNumber::from(2);
        // verify that g is generator in Z/q
        assert_eq!(g.gcd(&q), 1.into());
        let (ciphertext_orig, _) = key0.encrypt(plaintext_orig.to_bytes(), None).unwrap();
        let ciphertext_mult = g.modpow(&plaintext_mult, &q);
        let (ciphertext_add, nonce_y) = key1.encrypt(plaintext_add.to_bytes(), None).unwrap();
        let (ciphertext_add_action, nonce) = key0.encrypt(plaintext_add.to_bytes(), None).unwrap();
        // verify that D is obtained from affine transformation of C
        let transformed = key0
            .add(
                &key0.mul(&ciphertext_orig, &plaintext_mult).unwrap(),
                &ciphertext_add_action,
            )
            .unwrap();
        let data = super::Data {
            g,
            q,
            key0,
            key1,
            c: ciphertext_orig,
            d: transformed,
            y: ciphertext_add,
            x: ciphertext_mult,
        };
        let pdata = super::PrivateData {
            x: plaintext_mult,
            y: plaintext_add,
            nonce,
            nonce_y,
        };

        let p = BigNumber::prime(L + EPSILON + 1);
        let q = BigNumber::prime(L + EPSILON + 1);
        let rsa_modulo = p * q;
        let s: BigNumber = 123.into();
        let t: BigNumber = 321.into();
        assert_eq!(s.gcd(&rsa_modulo), 1.into());
        assert_eq!(t.gcd(&rsa_modulo), 1.into());
        let aux = super::Aux { s, t, rsa_modulo };

        let (commitment, challenge, proof) =
            super::compute_proof(&aux, &data, &pdata, rand_core::OsRng::default());
        let r = super::verify(&aux, &data, &commitment, &challenge, &proof);
        match r {
            Ok(()) => panic!("proof should not pass"),
            Err(_) => (),
        }
    }
}
