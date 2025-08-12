## v0.4.3
* Add a missing check that has been reported to lead to a security vulnerability

> [!WARNING]  
> This library version uses Zero-Knowledge proofs from the CGGMP21 paper, which contains a critical
> vulnerability that could lead to full private key recovery.
> 
> While we have patched this specific high-severity issue, the build still lacks other important
> security checks introduced in the revised CGGMP24 paper. The absence of these checks may expose
> other security risks.
> 
> For complete protection, please upgrade to the latest version of paillier-zk, which fully implements
> the more secure CGGMP24 revision.

## v0.4.2
* Update links in the crate settings, update readme [#53]

[#53]: https://github.com/LFDT-Lockness/paillier-zk/pull/53

## v0.4.1
* Prettify code by using `#[udigest(as = ...)]` attribute [#51]

[#51]: https://github.com/LFDT-Lockness/paillier-zk/pull/51

## v0.4.0
* security fix: derive challenges for zero-knowledge proof unambiguously

## v0.3.0
* Update `generic-ec` dep to v0.3 [#48]

[#48]: https://github.com/LFDT-Lockness/paillier-zk/pull/48

## v0.2.0

All changes prior to this version were not documented
