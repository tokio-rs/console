<a name="0.1.2"></a>
## 0.1.2 (2022-02-18)


#### Bug Fixes

*  console-api dependencies to require 0.1.2 (#274) ([b95f683f](b95f683f), closes [#270](270))
*  missing histogram in task details (#296) ([884f4eca](884f4eca), closes [#296](296))

  
<a name="0.1.1"></a>
## 0.1.1 (2022-01-18)


#### Features

*  feature-flag `tracing-journald` dependency (#250) ([24f25dbd](24f25dbd))
*  add vi style keybinds for tables (#223) ([1845c998](1845c998))

#### Bug Fixes

*  fix task lookup in async ops view (#257) ([9a50b630](9a50b630))
*  don't make details requests with rewritten IDs (#251) ([4ec26a8d](4ec26a8d))
*  fix build error with journald enabled ([a931b7ec](a931b7ec))
*  increase default event buffer capacity a bit (#235) ([0cf0aee3](0cf0aee3))
*  wrap controls line when the terminal is too narrow (#231) ([ef415072](ef415072))
*  don't enable crossterm mouse capture (#222) ([e020d66c](e020d66c), closes [#167](167))


#### Changes

*  move ID rewriting from `console-subscriber` to the client (#244) ([095b1ef](095b1ef))

<a name="0.1.0"></a>
## 0.1.0 (2021-12-16)

- Initial release! &#x1f389;
