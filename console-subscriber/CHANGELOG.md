<a name="0.1.6"></a>
## 0.1.6 (2022-05-23)


#### Features

*  add `Builder::poll_duration_histogram_max` (#351) ([a966feb3](a966feb3))

#### Bug Fixes

*  fix memory leak from resizing histograms (#351) ([32dd3376](32dd3376), closes [#350](350))


<a name="0.1.5"></a>
## 0.1.5 (2022-04-30)


#### Features


*  add support for `EnvFilter` in `Builder::init` (#337) ([1fe84b72](1fe84b72))

<a name="0.1.4"></a>
## 0.1.4  (2022-04-11)


#### Bug Fixes

*  fix memory leak from historical `PollOp`s (#311) ([9178ecf0](9178ecf0), closes [#256](256))


#### Features

* **console-api:**  Update `tonic` to `0.7` (#318) ([83d8a870](83d8a870))
*  don't trace tasks spawned through the console server (#314) ([0045e9bf](0045e9bf))


<a name="0.1.3"></a>
## 0.1.3  (2022-02-18)


#### Features

*  add `Builder::filter_env_var` builder parameter (#276) ([dbdb1494](dbdb1494), closes [#206](206))

#### Bug Fixes

*  record timestamps for updates last (#289) ([703f1aa4](703f1aa4), closes [#266](266))
*  use monotonic `Instant`s for all timestamps (#288) ([abc08300](abc08300), closes [#286](286))
*  bail rather than panic when encountering clock skew (#287) ([24db8c60](24db8c60), closes [#286](286))
*  fix compilation on targets without 64-bit atomics (#282) ([5590fdbc](5590fdbc), closes [#279](279))

  
<a name="0.1.2"></a>
## 0.1.2 (2022-01-18)


#### Bug Fixes

*  update console-api dependencies to require 0.1.2 (#274) ([b95f683f](b95f683f))


<a name="0.1.1"></a>
## 0.1.1 (2022-01-18)


#### Bug Fixes

*  only send *new* tasks/resources/etc over the event channel (#238) ([fdc77e28](fdc77e28))
*  increased default event buffer capacity (#235) ([0cf0aee](0cf0aee))
*  use saturating arithmetic for attribute updates (#234) ([fe82e170](fe82e170))

#### Changes

*  moved ID rewriting from `console-subscriber` to the client (#244) ([095b1ef](095b1ef))

## 0.1.0 (2021-12-16)


- Initial release! &#x1f389;
