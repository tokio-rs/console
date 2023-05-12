# Changelog

All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## tokio-console-v0.1.8 - (2023-05-09)

[3bf60bc](https://github.com/tokio-rs/console/commit/3bf60bce7b478c189a3145311e06f14cb2fc1e11)...[c8a69e9](https://github.com/tokio-rs/console/commit/c8a69e9269c113bdde909ad1f4aab4156adfd5f4)


### Added

- Reduce decimal digits in UI ([#402](https://github.com/tokio-rs/console/issues/402)) ([57b866d](https://github.com/tokio-rs/console/commit/57b866dd70ee36545ea0c02b02be872183cfa431))
- Use tokio task ids in task views ([#403](https://github.com/tokio-rs/console/issues/403)) ([001fc49](https://github.com/tokio-rs/console/commit/001fc49f09ad78cc4ab50770cf4a677ae177103f))
- Add support for Unix domain sockets ([#388](https://github.com/tokio-rs/console/issues/388)) ([bff8b8a](https://github.com/tokio-rs/console/commit/bff8b8a4291b0584ab4f97c5f91246eb9a68f262), closes [#296](https://github.com/tokio-rs/console/issues/296))
- Add scheduled time per task ([#406](https://github.com/tokio-rs/console/issues/406)) ([ac20daa](https://github.com/tokio-rs/console/commit/ac20daaf301f80e87002593813965d11d11371e4))
- Add task scheduled times histogram ([#409](https://github.com/tokio-rs/console/issues/409)) ([3b37dda](https://github.com/tokio-rs/console/commit/3b37dda773f8cd237f6759d193fdc83a75ab7653))
- Update `tonic` to 0.9 ([#420](https://github.com/tokio-rs/console/issues/420)) ([54f6be9](https://github.com/tokio-rs/console/commit/54f6be985a248d3dd5a98a7624a2447d0547bc60))
- Update MSRV to Rust 1.60.0 ([e3c5656](https://github.com/tokio-rs/console/commit/e3c56561a062be123be460dd477f512a6a9ec3cd))

### Documented

- Update screenshots in README ([#419](https://github.com/tokio-rs/console/issues/419)) ([4f71484](https://github.com/tokio-rs/console/commit/4f714845f3cda0978b0b1cc850072878ed4f8599))
- Revert "update screenshots in README ([#419](https://github.com/tokio-rs/console/issues/419))" ([7b86f7f](https://github.com/tokio-rs/console/commit/7b86f7f7d22d71ad7b677cdb2634c142c9bf5206))
- Update screenshots in README ([#421](https://github.com/tokio-rs/console/issues/421)) ([f4d3213](https://github.com/tokio-rs/console/commit/f4d321397355a6e458e170e174ac420c26e6353a))

### Fixed

- Fix ascii-only flipped input ([#377](https://github.com/tokio-rs/console/issues/377)) ([da0e972](https://github.com/tokio-rs/console/commit/da0e9724fa132595e2085cfb08ac7bfbf10542ba))
- Declare `tokio-console` bin as `default-run` ([#379](https://github.com/tokio-rs/console/issues/379)) ([40f7971](https://github.com/tokio-rs/console/commit/40f7971d30451f7321b73a03222b71731dabc52a))
- Make `retain_for` default to 6s if not specfied ([#383](https://github.com/tokio-rs/console/issues/383)) ([3248caa](https://github.com/tokio-rs/console/commit/3248caa8f8551e22c9d361e23cabd3c98aa143b6), fixes [#382](https://github.com/tokio-rs/console/issues/382))
- Enable view-switching keystrokes on details views ([#387](https://github.com/tokio-rs/console/issues/387)) ([d98f159](https://github.com/tokio-rs/console/commit/d98f15956075a2d64f5cb96b1011eff7b3110e51))
- Fix `ViewOptions` default lang' ([#394](https://github.com/tokio-rs/console/issues/394)) ([a7548d0](https://github.com/tokio-rs/console/commit/a7548d089812ac61602a31a699d14777d312ac6d), fixes [#393](https://github.com/tokio-rs/console/issues/393))
- Fix calculation of busy time during poll ([#405](https://github.com/tokio-rs/console/issues/405)) ([6fa2185](https://github.com/tokio-rs/console/commit/6fa2185134c8791446a1f1b5dc2ee11d254966ad))

## tokio-console-v0.1.7 - (2022-08-10)

[ce901c8](https://github.com/tokio-rs/console/commit/ce901c8f359d0de99430b51abd0cde63513de66a)...[3bf60bc](https://github.com/tokio-rs/console/commit/3bf60bce7b478c189a3145311e06f14cb2fc1e11)


### Added

- Emit a parse error a config file contains unknown fields ([#330](https://github.com/tokio-rs/console/issues/330)) ([3a67d47](https://github.com/tokio-rs/console/commit/3a67d476835e4a1d3557190b85f5a89c760490bb))
- Add missing configurations to config file ([#334](https://github.com/tokio-rs/console/issues/334)) ([472ff52](https://github.com/tokio-rs/console/commit/472ff52e6445dd2c103b218d60a4e0cad9a1972e), closes [#331](https://github.com/tokio-rs/console/issues/331))
- Display outliers in histogram view ([#351](https://github.com/tokio-rs/console/issues/351)) ([dec891f](https://github.com/tokio-rs/console/commit/dec891ff080b135dc10919b2b59665989e73daf3))
- Add subcommand to gen shell completions ([#336](https://github.com/tokio-rs/console/issues/336)) ([df4d468](https://github.com/tokio-rs/console/commit/df4d468375138b4115fb489b67ca72fbbd8f9ba1))
- Only suggest opening issues for panics ([#365](https://github.com/tokio-rs/console/issues/365)) ([23cb6bf](https://github.com/tokio-rs/console/commit/23cb6bf7cdaafd3fe691e4a6f7f91cc17e169795))
- Init error handling before subcmds ([#365](https://github.com/tokio-rs/console/issues/365)) ([6646568](https://github.com/tokio-rs/console/commit/66465689dceec509d9e1e37a55646a89285005e3))
- Filter out boring frames in backtraces ([#365](https://github.com/tokio-rs/console/issues/365)) ([523a44a](https://github.com/tokio-rs/console/commit/523a44a30cf047fe0a56f746624df0cc3239a160))
- Include config options in autogenerated issues ([#365](https://github.com/tokio-rs/console/issues/365)) ([fcb54df](https://github.com/tokio-rs/console/commit/fcb54dffda2a9f4c85cc82a24bff26e0777ceacc))

### Documented

- Update minimal Rust version ([#338](https://github.com/tokio-rs/console/issues/338)) ([ff3b6db](https://github.com/tokio-rs/console/commit/ff3b6db6fa06456a14992663e8ff7ba8c80c1cc1))

### Fixed

- Always log to a file instead of `stderr` ([#340](https://github.com/tokio-rs/console/issues/340)) ([ef39b9a](https://github.com/tokio-rs/console/commit/ef39b9a6419227d10e7a4d299ca95673ea200944), fixes [#339](https://github.com/tokio-rs/console/issues/339))
- Default `--no_colors` to `false` ([#344](https://github.com/tokio-rs/console/issues/344)) ([e58352f](https://github.com/tokio-rs/console/commit/e58352fe5e205620f1fe43acaceaff9cf7913394))

## tokio-console-v0.1.4 - (2022-04-13)

[3c55912](https://github.com/tokio-rs/console/commit/3c559121e3c5ad175471718a3cf87ada0146a7cd)...[ce901c8](https://github.com/tokio-rs/console/commit/ce901c8f359d0de99430b51abd0cde63513de66a)

### <a id = "tokio-console-v0.1.4-breaking"></a>Breaking Changes
- **Update `tonic` to `0.7` ([#318](https://github.com/tokio-rs/console/issues/318))** ([83d8a87](https://github.com/tokio-rs/console/commit/83d8a870bcc40be71bc23d0f45fc374899c636a8))<br />`console-api` is now no longer compatible with projects using `prost`
0.9 or `tonic` 0.7. These crates must be updated to use `console-api`
0.2.

### Added

- [**breaking**](#tokio-console-v0.1.4-breaking) Update `tonic` to `0.7` ([#318](https://github.com/tokio-rs/console/issues/318)) ([83d8a87](https://github.com/tokio-rs/console/commit/83d8a870bcc40be71bc23d0f45fc374899c636a8))
- Read configuration options from a config file ([#320](https://github.com/tokio-rs/console/issues/320)) ([defe346](https://github.com/tokio-rs/console/commit/defe34609508086cdc527fbf813cbca4732d49cd), closes [#310](https://github.com/tokio-rs/console/issues/310))
- Surface dropped event count if there are any ([#316](https://github.com/tokio-rs/console/issues/316)) ([16df5d3](https://github.com/tokio-rs/console/commit/16df5d30a011fc627e993ed71889877e70192baf))
- Add `gen-config` subcommand to generate a config file ([#324](https://github.com/tokio-rs/console/issues/324)) ([e034f8d](https://github.com/tokio-rs/console/commit/e034f8d0967fc589e4a48425075b6ae9abb47fc8))
- Add autogenerated example config file to docs ([#327](https://github.com/tokio-rs/console/issues/327)) ([79da280](https://github.com/tokio-rs/console/commit/79da280f4df9d1e0e6e3940e40b3faf123435a74))

### Documented

- Add tokio-console installation section ([#313](https://github.com/tokio-rs/console/issues/313)) ([d793903](https://github.com/tokio-rs/console/commit/d79390303590a534a8224efbe96c6023c336a32f))

## tokio-console-v0.1.3 - (2022-03-09)

[900a5c2](https://github.com/tokio-rs/console/commit/900a5c2bd5b610e9b939a5f824af1ac1a11267d0)...[3c55912](https://github.com/tokio-rs/console/commit/3c559121e3c5ad175471718a3cf87ada0146a7cd)


### Added

- Add icon representing column sorting state ([#301](https://github.com/tokio-rs/console/issues/301)) ([b9e0a22](https://github.com/tokio-rs/console/commit/b9e0a2266c98cd11a5261323dc20e04f17514b97))

### Fixed

- Prevent panics if subscriber reports out-of-order times ([#295](https://github.com/tokio-rs/console/issues/295)) ([80d7f42](https://github.com/tokio-rs/console/commit/80d7f4250ee5add0965ff100668be21d20621114))
- Exit crossterm before printing panic messages ([#307](https://github.com/tokio-rs/console/issues/307)) ([43606b9](https://github.com/tokio-rs/console/commit/43606b9a7ccff0157325effbc48e1d71a194e5de))

## tokio-console-v0.1.2 - (2022-02-18)

[e7b228d](https://github.com/tokio-rs/console/commit/e7b228d13b5da3885532ff5d42d7f41c90dcbcb0)...[900a5c2](https://github.com/tokio-rs/console/commit/900a5c2bd5b610e9b939a5f824af1ac1a11267d0)


### Added

- Fix missing histogram in task details ([#269](https://github.com/tokio-rs/console/issues/269)) ([884f4ec](https://github.com/tokio-rs/console/commit/884f4ecac8cba7eee7f895024da4c6e28de75289))

### Documented

- Document minimum Tokio versions ([#291](https://github.com/tokio-rs/console/issues/291)) ([3b1f14a](https://github.com/tokio-rs/console/commit/3b1f14a50c507e7b5b672491fada6dfb067fc671), closes [#281](https://github.com/tokio-rs/console/issues/281))

### Fixed

- Console-api dependencies to require 0.1.2 ([#274](https://github.com/tokio-rs/console/issues/274)) ([b95f683](https://github.com/tokio-rs/console/commit/b95f683f0514978429535a75c86f8974b05a69aa))

<!-- generated by git-cliff -->
