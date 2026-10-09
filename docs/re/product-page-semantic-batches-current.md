# 全产品页面语义覆盖批次（当前源）

本表从 [product-ui-details-current.json.gz](product-ui-details-current.json.gz) 逐产品流式生成，覆盖 331 个产品、1452 个页面。它为每个页面固定记录原组件根范围、UI 调用、事件回调候选、状态/订阅调用、条件/循环和 graph unresolved；所有页面都保留为 `partial` 或 `unknown`，没有把共享组件引用、JSX 统计或字符串匹配误报成“整个页面已逆向”。

## 口径

- `unknown`：没有保留根/UI 证据。
- `partial`：有原文根/UI 证据，但尚未闭合 mount → 控件 → 状态/回调 → service/DLL → 返回状态。
- `closed`：本生成器永不发出，避免把静态候选当作运行结果。
- 每个 `roots[].path/sha256/offset/end` 及 event/state evidence 都可以回到当前源字节；offset 是 UTF-16 左闭右开。事件名、method、callback 都是语法候选，不能证明用户动作被执行或写回设备。
- `graph_unresolved_count`、`graph_truncated` 被原样保留；DLL 查询/写回、host service、动态 import、HOC 参数和 CSS 渲染仍需逐页来源审阅。

## 汇总

| 指标 | 数值 |
|---|---:|
| 产品 | 331 |
| 页面 | 1452 |
| 页面组件引用 | 52461 |
| UI 调用候选 | 413492 |
| 事件候选 | 82465 |
| 状态/订阅调用 | 110024 |
| 条件节点 | 1074164 |
| 循环/迭代节点 | 38314 |
| 有 unresolved 的页面 | 854 |
| partial | 1452 |
| unknown | 0 |
| semantic_complete_pages_claimed | 0 |

## 产品批次

| 产品 ID | 名称 | 页面 | partial | unknown | UI 调用 | 事件候选 | 状态调用 | unresolved |
|---:|---|---:|---:|---:|---:|---:|---:|---:|
| 70 | Razer Mamba TE | 5 | 5 | 0 | 1577 | 285 | 345 | 8 |
| 80 | Razer Naga Hex V2 | 5 | 5 | 0 | 1609 | 297 | 361 | 8 |
| 83 | Razer Naga Chroma | 5 | 5 | 0 | 1616 | 297 | 360 | 8 |
| 89 | Razer Lancehead | 6 | 6 | 0 | 1645 | 303 | 360 | 8 |
| 92 | Razer Deathadder Elite | 5 | 5 | 0 | 1575 | 285 | 345 | 54 |
| 96 | Razer Lancehead TE | 5 | 5 | 0 | 1569 | 285 | 335 | 8 |
| 98 | Razer Atheris | 5 | 5 | 0 | 1386 | 250 | 300 | 4 |
| 99 | Razer Jugan | 5 | 5 | 0 | 1564 | 285 | 335 | 8 |
| 100 | Razer Basilisk | 5 | 5 | 0 | 1576 | 285 | 345 | 8 |
| 101 | Razer Basilisk | 5 | 5 | 0 | 1575 | 285 | 345 | 8 |
| 103 | RAZER NAGA TRINITY | 5 | 5 | 0 | 1647 | 297 | 371 | 8 |
| 104 | Razer Mamba Hyperflux | 5 | 5 | 0 | 1634 | 302 | 379 | 8 |
| 105 | Razer Mamba Hyperflux | 5 | 5 | 0 | 1634 | 302 | 379 | 8 |
| 106 | D.VA Razer Abyssus Elite | 5 | 5 | 0 | 1571 | 285 | 345 | 54 |
| 107 | Razer Abyssus Essential | 5 | 5 | 0 | 1571 | 285 | 345 | 8 |
| 108 | Razer Mamba Elite | 5 | 5 | 0 | 1577 | 285 | 345 | 8 |
| 110 | Razer DeathAdder Essential | 4 | 4 | 0 | 1020 | 241 | 285 | 4 |
| 112 | Razer Lancehead Wireless | 6 | 6 | 0 | 1645 | 303 | 360 | 8 |
| 113 | Razer DeathAdder Essential | 4 | 4 | 0 | 1025 | 241 | 293 | 4 |
| 115 | Razer Mamaba Wireless | 6 | 6 | 0 | 1643 | 303 | 360 | 8 |
| 116 | Razer Abyssus Lite | 5 | 5 | 0 | 1571 | 285 | 345 | 8 |
| 117 | Razer Turret Mouse Xbox One Edition | 6 | 6 | 0 | 1655 | 305 | 357 | 8 |
| 120 | Razer Viper | 6 | 6 | 0 | 1843 | 328 | 391 | 8 |
| 122 | Razer Viper Ultimate | 6 | 6 | 0 | 1791 | 323 | 392 | 8 |
| 124 | Razer DeathAdder V2 Pro | 5 | 5 | 0 | 1439 | 297 | 394 | 11 |
| 126 | Razer Mouse Dock | 2 | 2 | 0 | 620 | 100 | 117 | 4 |
| 128 | Razer Pro Click | 4 | 4 | 0 | 985 | 227 | 271 | 4 |
| 131 | Razer Basilisk X Hyperspeed | 5 | 5 | 0 | 1003 | 230 | 293 | 4 |
| 132 | RAZER DEATHADDER V2 | 5 | 5 | 0 | 1734 | 307 | 379 | 8 |
| 133 | Razer Basilisk V2 | 5 | 5 | 0 | 1727 | 305 | 377 | 8 |
| 134 | Razer Basilisk Ultimate | 6 | 6 | 0 | 1794 | 323 | 391 | 8 |
| 138 | Razer Viper Mini | 5 | 5 | 0 | 1574 | 285 | 345 | 8 |
| 140 | Razer Deathadder V2 Lite | 5 | 5 | 0 | 1573 | 285 | 350 | 8 |
| 141 | Razer Naga Left Handed Edition | 5 | 5 | 0 | 1775 | 319 | 390 | 8 |
| 143 | RAZER NAGA PRO | 5 | 5 | 0 | 1514 | 314 | 419 | 11 |
| 145 | Razer Viper 8Khz | 5 | 5 | 0 | 1724 | 305 | 377 | 8 |
| 147 | RAZER NAGA CLASSIC EDITION | 5 | 5 | 0 | 1648 | 297 | 371 | 8 |
| 148 | Razer Orochi V2 | 5 | 5 | 0 | 1180 | 245 | 334 | 7 |
| 150 | Razer Naga X | 5 | 5 | 0 | 1615 | 298 | 368 | 8 |
| 152 | Razer DeathAdder Essential | 4 | 4 | 0 | 1026 | 241 | 293 | 4 |
| 153 | Razer Basilisk V3 | 5 | 5 | 0 | 1759 | 311 | 378 | 8 |
| 154 | Razer Pro Click Mini | 4 | 4 | 0 | 1156 | 242 | 314 | 7 |
| 156 | Razer DeathAdder V2 X Hyperspeed | 5 | 5 | 0 | 1174 | 245 | 333 | 7 |
| 158 | Razer Viper Mini Signature Edition | 6 | 6 | 0 | 1372 | 259 | 326 | 4 |
| 161 | Razer Deathadder V2 Lite | 5 | 5 | 0 | 1573 | 285 | 350 | 8 |
| 162 | cobra | 5 | 5 | 0 | 529 | 38 | 43 | 67 |
| 163 | Razer Cobra | 5 | 5 | 0 | 1574 | 285 | 345 | 8 |
| 164 | Razer Mouse Dock Pro | 4 | 4 | 0 | 754 | 138 | 169 | 7 |
| 165 | RAZER VIPER V2 PRO | 5 | 5 | 0 | 1313 | 268 | 348 | 7 |
| 167 | RAZER NAGA V2 PRO | 7 | 7 | 0 | 1809 | 389 | 488 | 11 |
| 170 | Razer Basilisk V3 Pro | 7 | 7 | 0 | 1643 | 332 | 437 | 11 |
| 175 | Razer Cobra Pro | 6 | 6 | 0 | 1466 | 301 | 410 | 11 |
| 178 | Razer DeathAdder V3 | 4 | 4 | 0 | 1081 | 235 | 299 | 4 |
| 179 | HyperPolling Wireless Dongle | 2 | 2 | 0 | 555 | 66 | 66 | 1 |
| 180 | Razer Naga V2 Hyperspeed | 5 | 5 | 0 | 1224 | 257 | 354 | 7 |
| 182 | Razer DeathAdder V3 Pro | 6 | 6 | 0 | 1333 | 271 | 368 | 7 |
| 184 | Razer Viper V3 HyperSpeed | 6 | 6 | 0 | 1341 | 277 | 361 | 7 |
| 185 | Razer Basilisk V3 X Hyperspeed | 7 | 7 | 0 | 1894 | 338 | 439 | 11 |
| 190 | Razer DeathAdder V4 Pro | 7 | 7 | 0 | 1705 | 346 | 473 | 4 |
| 192 | Razer Viper V3 Pro | 7 | 7 | 0 | 1804 | 335 | 459 | 4 |
| 194 | Razer DeathAdder V3 Pro Hyperpolling Technology | 5 | 5 | 0 | 1358 | 256 | 314 | 4 |
| 196 | Razer DeathAdder V3 HyperSpeed | 7 | 7 | 0 | 1513 | 307 | 444 | 7 |
| 199 | Razer Pro Click V2 Vertical Edition | 4 | 4 | 0 | 1312 | 264 | 346 | 18 |
| 203 | Razer Basilisk V3 35K | 5 | 5 | 0 | 1562 | 334 | 405 | 8 |
| 204 | Razer Basilisk V3 Pro 35K | 7 | 7 | 0 | 1843 | 373 | 482 | 11 |
| 207 | HyperFlux V2 Wireless Charging System | 3 | 3 | 0 | 664 | 103 | 156 | 3 |
| 208 | Razer Pro Click V2 | 4 | 4 | 0 | 1318 | 264 | 346 | 14 |
| 211 | Razer Basilisk Mobile | 6 | 6 | 0 | 1459 | 301 | 402 | 11 |
| 214 | Razer Basilisk V3 Pro 35K Phantom Green Edition | 7 | 7 | 0 | 1863 | 373 | 482 | 11 |
| 218 | Razer Cobra HyperSpeed | 6 | 6 | 0 | 1466 | 301 | 410 | 11 |
| 221 | Razer Boomslang 20th Anniversary Edition | 7 | 7 | 0 | 1287 | 257 | 341 | 37 |
| 222 | Razer Viper V3 Pro SE | 7 | 7 | 0 | 1763 | 347 | 501 | 7 |
| 224 | Razer Orochi V2 | 4 | 4 | 0 | 1014 | 233 | 284 | 4 |
| 226 | Razer Basilisk V4 Pro | 7 | 7 | 0 | 2080 | 393 | 546 | 16 |
| 229 | Razer Viper V4 Pro | 6 | 6 | 0 | 1626 | 302 | 412 | 4 |
| 231 | RAZER NAGA V3 PRO | 5 | 5 | 0 | 1739 | 372 | 450 | 11 |
| 235 | Razer Basilisk V4 HyperSpeed | 6 | 6 | 0 | 966 | 189 | 204 | 15 |
| 239 | Razer DeathAdder V4 Pro Carbon Fiber Edition | 6 | 6 | 0 | 1691 | 339 | 458 | 4 |
| 241 | Razer Mouse Dock V2 Pro | 3 | 3 | 0 | 729 | 125 | 158 | 6 |
| 515 | Razer Blackwidow Chroma | 3 | 3 | 0 | 1086 | 233 | 352 | 9 |
| 521 | Razer Blackwidow TE Chroma V2 | 3 | 3 | 0 | 1098 | 238 | 353 | 9 |
| 529 | Razer Blackwidow Chroma | 3 | 3 | 0 | 1086 | 233 | 352 | 9 |
| 534 | Razer Blackwidow X Chroma | 3 | 3 | 0 | 1098 | 238 | 353 | 9 |
| 542 | Razer Ornata Chroma | 3 | 3 | 0 | 1098 | 238 | 353 | 9 |
| 545 | Razer BlackWidow Chroma V2 | 3 | 3 | 0 | 1079 | 233 | 343 | 9 |
| 550 | Razer Huntsman Elite | 3 | 3 | 0 | 3 | 0 | 9 | 3 |
| 551 | Razer Huntsman | 3 | 3 | 0 | 1098 | 238 | 353 | 9 |
| 552 | Razer Blackwidow Elite | 3 | 3 | 0 | 1091 | 238 | 344 | 9 |
| 554 | Razer Cynosa Chroma | 3 | 3 | 0 | 1098 | 238 | 353 | 9 |
| 555 | RAZER TARTARUS V2 | 3 | 3 | 0 | 1176 | 224 | 325 | 9 |
| 556 | Razer Cynosa Chroma Pro | 3 | 3 | 0 | 1098 | 238 | 353 | 9 |
| 563 | Razer Blade 15 | 4 | 4 | 0 | 1646 | 347 | 482 | 9 |
| 564 | Razer Blade Pro 17 | 4 | 4 | 0 | 1717 | 352 | 488 | 9 |
| 565 | Razer Blackwidow Lite | 3 | 3 | 0 | 885 | 199 | 293 | 5 |
| 567 | Razer Blackwidow Essential | 3 | 3 | 0 | 928 | 212 | 318 | 5 |
| 569 | Razer Blade Stealth 13 | 4 | 4 | 0 | 1646 | 347 | 482 | 9 |
| 570 | Razer Blade 15 | 4 | 4 | 0 | 1646 | 347 | 482 | 9 |
| 571 | Razer Blade 15 | 4 | 4 | 0 | 1615 | 345 | 478 | 9 |
| 574 | Turret Keyboard Xbox One Edition | 4 | 4 | 0 | 1163 | 253 | 373 | 9 |
| 575 | Razer Cynosa Lite | 3 | 3 | 0 | 1086 | 233 | 352 | 9 |
| 576 | Razer Blade 15 | 4 | 4 | 0 | 1646 | 347 | 482 | 9 |
| 577 | Razer BlackWidow | 3 | 3 | 0 | 1091 | 238 | 344 | 9 |
| 579 | Razer Huntsman Tournament Edition | 3 | 3 | 0 | 1109 | 240 | 357 | 9 |
| 580 | Tartarus Pro | 4 | 4 | 0 | 1486 | 277 | 438 | 9 |
| 581 | Razer Blade 15 | 4 | 4 | 0 | 1646 | 347 | 482 | 9 |
| 582 | Razer Blade 15 | 4 | 4 | 0 | 1646 | 347 | 482 | 9 |
| 585 | Razer Pro Type | 4 | 4 | 0 | 964 | 221 | 313 | 5 |
| 586 | Razer Blade Stealth 13 | 4 | 4 | 0 | 1646 | 347 | 482 | 9 |
| 587 | Razer Blade 15 Advanced Model | 4 | 4 | 0 | 1717 | 352 | 488 | 9 |
| 588 | Razer Blade Pro 17 | 4 | 4 | 0 | 1717 | 352 | 488 | 9 |
| 589 | Razer Blade 15 Studio Edition | 4 | 4 | 0 | 1646 | 347 | 482 | 9 |
| 590 | Razer BlackWidow V3 | 3 | 3 | 0 | 1116 | 240 | 359 | 9 |
| 591 | Razer BlackWidow X: Tenkeyless | 3 | 3 | 0 | 940 | 217 | 319 | 5 |
| 592 | Razer Huntsman Essential | 3 | 3 | 0 | 940 | 217 | 319 | 5 |
| 593 | Razer Pokémon | 3 | 3 | 0 | 940 | 217 | 319 | 5 |
| 594 | Razer Blade Stealth 13 Base Model | 4 | 4 | 0 | 1698 | 357 | 498 | 9 |
| 595 | Razer Blade 15 Advanced Model | 4 | 4 | 0 | 1769 | 362 | 504 | 9 |
| 597 | Razer Blade 15 Base Model | 4 | 4 | 0 | 1698 | 357 | 498 | 9 |
| 598 | Razer Blade Pro 17 | 4 | 4 | 0 | 1769 | 362 | 504 | 9 |
| 599 | Razer Huntsman Mini | 3 | 3 | 0 | 1109 | 240 | 357 | 9 |
| 600 | Razer BlackWidow V3 Mini HyperSpeed | 5 | 5 | 0 | 1351 | 270 | 429 | 12 |
| 601 | Razer Blade Stealth 13 | 4 | 4 | 0 | 1698 | 357 | 498 | 9 |
| 602 | Blackwidow V3 Pro | 5 | 5 | 0 | 1363 | 275 | 430 | 12 |
| 605 | Razer Ornata V2 | 3 | 3 | 0 | 1109 | 240 | 357 | 9 |
| 606 | RAZER CYNOSA V2 | 3 | 3 | 0 | 1099 | 238 | 355 | 9 |
| 614 | Razer Huntsman V2 Analog | 4 | 4 | 0 | 1554 | 305 | 491 | 9 |
| 616 | Razer Blade 15 Base Model | 4 | 4 | 0 | 1698 | 357 | 498 | 9 |
| 617 | Razer Huntsman Mini | 3 | 3 | 0 | 1109 | 240 | 357 | 9 |
| 618 | Razer Book 13 | 4 | 4 | 0 | 1698 | 357 | 498 | 9 |
| 619 | Razer Huntsman V2 Tenkeyless | 3 | 3 | 0 | 1144 | 245 | 369 | 9 |
| 620 | Razer Huntsman V2 | 3 | 3 | 0 | 1146 | 245 | 371 | 9 |
| 621 | Razer Blade 15 Advanced Model | 4 | 4 | 0 | 1783 | 363 | 504 | 9 |
| 622 | Razer Blade Pro 17 | 4 | 4 | 0 | 1769 | 362 | 504 | 9 |
| 623 | Razer Blade 15 Base Model | 4 | 4 | 0 | 1698 | 357 | 498 | 9 |
| 624 | Razer Blade 14 | 4 | 4 | 0 | 1698 | 357 | 498 | 9 |
| 630 | Razer Blade 15 Advanced Model | 4 | 4 | 0 | 1779 | 362 | 502 | 9 |
| 631 | Razer Pro Type Ultra | 4 | 4 | 0 | 1148 | 238 | 358 | 8 |
| 633 | Razer Blade 17 | 4 | 4 | 0 | 1753 | 361 | 502 | 9 |
| 634 | Razer Blade 15 Base Model | 4 | 4 | 0 | 1769 | 362 | 504 | 9 |
| 642 | Razer Huntsman Mini Analog | 4 | 4 | 0 | 1574 | 305 | 487 | 9 |
| 647 | Razer BlackWidow V4 | 3 | 3 | 0 | 1134 | 244 | 370 | 9 |
| 650 | Razer Blade 15 Advanced Model | 5 | 5 | 0 | 1925 | 391 | 546 | 9 |
| 651 | Razer Blade 17 | 5 | 5 | 0 | 1895 | 389 | 544 | 9 |
| 652 | Razer Blade 14  | 5 | 5 | 0 | 1911 | 390 | 546 | 9 |
| 653 | Blackwidow V4 Pro | 3 | 3 | 0 | 1553 | 362 | 504 | 12 |
| 654 | Razer Cynosa Pro | 3 | 3 | 0 | 933 | 217 | 309 | 5 |
| 655 | Razer Ornata V3 | 3 | 3 | 0 | 1109 | 240 | 357 | 9 |
| 658 | Razer DeathStalker V2 Pro | 5 | 5 | 0 | 1371 | 275 | 430 | 12 |
| 659 | Razer BlackWidow V4 X | 3 | 3 | 0 | 1109 | 240 | 359 | 9 |
| 660 | Razer Ornata V3 | 3 | 3 | 0 | 1098 | 238 | 353 | 9 |
| 661 | Razer DeathStalker V2 Pro | 3 | 3 | 0 | 1116 | 240 | 359 | 9 |
| 664 | Razer DeathStalker V2 Pro Tenkeyless | 5 | 5 | 0 | 1370 | 275 | 432 | 12 |
| 669 | Razer Blade 14  | 6 | 6 | 0 | 2177 | 446 | 608 | 9 |
| 670 | Razer Blade 15 | 6 | 6 | 0 | 2010 | 414 | 562 | 9 |
| 671 | Razer Blade 16 | 6 | 6 | 0 | 2200 | 449 | 613 | 9 |
| 672 | Razer Blade 18 | 6 | 6 | 0 | 2200 | 449 | 613 | 9 |
| 673 | Razer Ornata V3 | 3 | 3 | 0 | 1109 | 240 | 357 | 9 |
| 674 | Razer Ornata V3 | 3 | 3 | 0 | 1098 | 238 | 353 | 9 |
| 675 | Razer Ornata V3 Tenkeyless | 3 | 3 | 0 | 1098 | 240 | 347 | 9 |
| 677 | Razer BlackWidow V4 75% | 3 | 3 | 0 | 1128 | 244 | 360 | 9 |
| 678 | Razer Huntsman V3 Pro | 4 | 4 | 0 | 1611 | 304 | 498 | 9 |
| 679 | Razer Huntsman V3 Pro Tenkeyless | 4 | 4 | 0 | 1610 | 304 | 498 | 9 |
| 688 | Razer Huntsman V3 Pro Mini | 4 | 4 | 0 | 1638 | 304 | 496 | 9 |
| 689 | RAZER HUNTSMAN V3 X TENKEYLESS | 3 | 3 | 0 | 1109 | 240 | 357 | 9 |
| 691 | Blackwidow V4 Pro 75% | 5 | 5 | 0 | 2167 | 476 | 572 | 104 |
| 694 | Razer Blade 14 | 7 | 7 | 0 | 1850 | 317 | 496 | 216 |
| 695 | Razer Blade 16 | 7 | 7 | 0 | 1873 | 320 | 501 | 219 |
| 696 | Razer Blade 18 | 7 | 7 | 0 | 1862 | 322 | 498 | 217 |
| 697 | Razer BlackWidow V3 Mini HyperSpeed | 5 | 5 | 0 | 1351 | 270 | 429 | 12 |
| 708 | Razer Pro Type Ergo | 4 | 4 | 0 | 1544 | 366 | 464 | 11 |
| 709 | Razer Blade 14 | 7 | 7 | 0 | 2643 | 541 | 818 | 118 |
| 710 | Razer Blade 16 | 7 | 7 | 0 | 2638 | 541 | 823 | 118 |
| 711 | Razer Blade 18 | 7 | 7 | 0 | 2650 | 546 | 821 | 118 |
| 716 | Razer BlackWidow V4 Low-profile HyperSpeed | 5 | 5 | 0 | 1401 | 279 | 438 | 12 |
| 717 | Razer Joro | 5 | 5 | 0 | 1217 | 259 | 387 | 9 |
| 719 | Razer Huntsman V3 Pro 8KHz | 4 | 4 | 0 | 1770 | 330 | 538 | 20 |
| 720 | Razer Huntsman V3 Pro Tenkeyless 8KHZ | 4 | 4 | 0 | 1772 | 330 | 537 | 20 |
| 721 | Razer Huntsman V3 Pro Mini 8KHz | 4 | 4 | 0 | 1786 | 327 | 534 | 15 |
| 724 | BlackWidow V4 Low-profile Tenkeyless HyperSpeed | 5 | 5 | 0 | 1400 | 279 | 438 | 12 |
| 727 | Razer BlackWidow V4 Tenkeyless HyperSpeed | 5 | 5 | 0 | 1389 | 279 | 428 | 12 |
| 728 | Razer Huntsman Signature Editon | 4 | 4 | 0 | 1797 | 330 | 533 | 21 |
| 736 | Razer Blade 16 | 7 | 7 | 0 | 2658 | 546 | 816 | 118 |
| 737 | Razer Blade 18 | 7 | 7 | 0 | 2658 | 546 | 811 | 118 |
| 739 | Razer Reclusa X Mini 65% | 3 | 3 | 0 | 1245 | 253 | 385 | 14 |
| 740 | Razer Huntsman V3 HE Magnetic Mini 65% 8KHz | 5 | 5 | 0 | 1965 | 371 | 614 | 18 |
| 741 | Razer Huntsman V3 Tenkeyless 8KHZ | 4 | 4 | 0 | 1773 | 330 | 536 | 19 |
| 742 | RAZER HUNTSMAN V3 PRO LOW-PROFILE TENKEYLESS 8KHZ | 4 | 4 | 0 | 1788 | 330 | 545 | 21 |
| 746 | Razer Huntsman V3 HE Magnetic Tenkeyless 8KHz | 5 | 5 | 0 | 1948 | 367 | 611 | 23 |
| 747 | Tartarus Pro | 4 | 4 | 0 | 1657 | 288 | 486 | 15 |
| 752 | Razer BlackWidow V4 75% XBOX 25 Anniversary Edition  | 3 | 3 | 0 | 1112 | 231 | 341 | 12 |
| 769 | RAZER PHILIPS HUE | 2 | 2 | 0 | 458 | 78 | 94 | 4 |
| 777 | RAZER KRAKEN BT SANRIO LIMITED EDITION | 5 | 5 | 0 | 821 | 166 | 185 | 37 |
| 778 | ASRock B550 Taichi Razer Edition | 3 | 3 | 0 | 520 | 103 | 128 | 6 |
| 780 | Razer Key Light Chroma | 2 | 2 | 0 | 661 | 151 | 150 | 1 |
| 781 | Razer Aether Lamp Pro | 2 | 2 | 0 | 674 | 151 | 158 | 1 |
| 782 | Razer Aether Lamp | 2 | 2 | 0 | 674 | 151 | 158 | 1 |
| 783 | Razer Aether Light Bulb | 2 | 2 | 0 | 680 | 151 | 158 | 1 |
| 784 | Razer Aether Light Strip | 3 | 3 | 0 | 694 | 150 | 154 | 3 |
| 790 | RAZER AETHER MONITOR LIGHT BAR | 2 | 2 | 0 | 701 | 156 | 160 | 1 |
| 791 | Razer Aether Standing Light Bars | 2 | 2 | 0 | 606 | 129 | 143 | 1 |
| 1303 | Razer Nommo Chroma | 3 | 3 | 0 | 676 | 127 | 144 | 4 |
| 1304 | Razer Nommo Pro | 3 | 3 | 0 | 708 | 134 | 157 | 4 |
| 1306 | RAZER NARI ULTIMATE | 7 | 7 | 0 | 1329 | 322 | 322 | 82 |
| 1308 | RAZER NARI | 7 | 7 | 0 | 1225 | 243 | 308 | 110 |
| 1310 | RAZER NARI ESSENTIAL | 6 | 6 | 0 | 977 | 191 | 249 | 80 |
| 1312 | RAZER USB AUDIO CONTROLLER | 5 | 5 | 0 | 855 | 163 | 218 | 66 |
| 1313 | Razer Kraken Kitty Edition | 6 | 6 | 0 | 1149 | 228 | 293 | 102 |
| 1318 | Razer Kraken X USB | 3 | 3 | 0 | 493 | 103 | 104 | 14 |
| 1319 | RAZER KRAKEN ULTIMATE | 6 | 6 | 0 | 1204 | 239 | 307 | 108 |
| 1320 | RAZER BLACKSHARK V2 PRO | 6 | 6 | 0 | 1016 | 203 | 263 | 83 |
| 1321 | Razer USB Sound Card | 5 | 5 | 0 | 947 | 187 | 247 | 77 |
| 1325 | RAZER KRAKEN V3 PRO | 7 | 7 | 0 | 1284 | 258 | 345 | 118 |
| 1328 | Razer Kraken BT Kitty Edition | 5 | 5 | 0 | 821 | 166 | 185 | 37 |
| 1330 | RAZER LEVIATHAN V2 | 4 | 4 | 0 | 886 | 162 | 247 | 73 |
| 1331 | RAZER KRAKEN V3 HYPERSENSE | 6 | 6 | 0 | 1207 | 241 | 309 | 109 |
| 1332 | RAZER KRAKEN BT SANRIO LIMITED EDITION | 5 | 5 | 0 | 821 | 166 | 185 | 37 |
| 1335 | Razer Kraken V3 X | 3 | 3 | 0 | 672 | 126 | 138 | 27 |
| 1337 | RAZER BARRACUDA PRO 2.4 | 6 | 6 | 0 | 974 | 204 | 254 | 84 |
| 1339 | Razer Barracuda 2.4 | 6 | 6 | 0 | 954 | 199 | 250 | 83 |
| 1342 | Razer Audio Mixer | 6 | 6 | 0 | 1330 | 240 | 336 | 40 |
| 1346 | Razer Seiren V2 Pro | 3 | 3 | 0 | 926 | 186 | 168 | 1 |
| 1347 | Razer Seiren V2 X | 3 | 3 | 0 | 926 | 186 | 168 | 1 |
| 1352 | Razer Leviathan V2 Pro | 5 | 5 | 0 | 996 | 238 | 222 | 28 |
| 1353 | RAZER KRAKEN V3 | 6 | 6 | 0 | 1165 | 239 | 308 | 105 |
| 1354 | Razer Leviathan V2 X | 4 | 4 | 0 | 822 | 164 | 191 | 5 |
| 1364 | Razer Kitty V2 Pro | 6 | 6 | 0 | 1201 | 240 | 309 | 111 |
| 1365 | RAZER BLACKSHARK V2 PRO | 6 | 6 | 0 | 1018 | 203 | 266 | 85 |
| 1370 | RAZER NOMMO V2 PRO | 5 | 5 | 0 | 970 | 182 | 269 | 84 |
| 1372 | RAZER NOMMO V2 | 5 | 5 | 0 | 989 | 186 | 282 | 85 |
| 1374 | RAZER NOMMO V2 X | 4 | 4 | 0 | 695 | 129 | 200 | 52 |
| 1376 | Razer Kraken Kitty V2 | 4 | 4 | 0 | 760 | 149 | 163 | 32 |
| 1378 | Razer Kraken Kitty V2 BT Quartz Edition | 4 | 4 | 0 | 752 | 144 | 157 | 31 |
| 1382 | Razer WIRELESS CONTROL POD | 2 | 2 | 0 | 934 | 160 | 231 | 31 |
| 1383 | Razer Kraken V4 Pro | 9 | 9 | 0 | 3030 | 723 | 850 | 149 |
| 1386 | Razer Seiren V3 Mini | 3 | 3 | 0 | 977 | 195 | 186 | 3 |
| 1387 | Razer Kraken V4 | 7 | 7 | 0 | 1204 | 207 | 309 | 139 |
| 1389 | Razer Kraken V4 X | 4 | 4 | 0 | 782 | 156 | 173 | 4 |
| 1390 | RAZER BLACKSHARK V2 HYPERSPEED | 6 | 6 | 0 | 1229 | 209 | 269 | 87 |
| 1391 | Razer Seiren V3 Chroma | 4 | 4 | 0 | 1326 | 236 | 261 | 7 |
| 1392 | RAZER CLIO | 5 | 5 | 0 | 814 | 141 | 185 | 63 |
| 1396 | Razer Barracuda X Chroma | 6 | 6 | 0 | 880 | 166 | 236 | 74 |
| 1398 | Razer BlackShark V3 Pro | 7 | 7 | 0 | 1714 | 337 | 382 | 155 |
| 1401 | Razer BlackShark V3 | 7 | 7 | 0 | 1699 | 334 | 382 | 154 |
| 1404 | Razer BlackShark V3 X Hyperspeed | 4 | 4 | 0 | 676 | 133 | 181 | 46 |
| 1406 | Razer Kraken V2 BT Hello Kitty and Friends Edition | 4 | 4 | 0 | 752 | 144 | 157 | 31 |
| 1407 | Razer \| Sanrio Characters Limited Edition Wireless Headset | 4 | 4 | 0 | 752 | 144 | 157 | 31 |
| 1411 | Razer Barracuda Pro | 6 | 6 | 0 | 1077 | 285 | 270 | 27 |
| 1412 | Razer Barracuda 2.4 | 6 | 6 | 0 | 1051 | 274 | 266 | 27 |
| 1415 | Razer Kraken Kitty V3 Pro | 7 | 7 | 0 | 1668 | 361 | 470 | 85 |
| 1420 | Razer Hammerhead V3 | 3 | 3 | 0 | 492 | 93 | 95 | 13 |
| 1422 | Razer Seiren V3 Pro | 5 | 5 | 0 | 1381 | 356 | 404 | 7 |
| 1427 | Razer Madeline T1 | 7 | 7 | 0 | 1762 | 456 | 539 | 64 |
| 1439 | Razer Kraken V4 X Sensa HD Haptics | 5 | 5 | 0 | 1298 | 273 | 349 | 7 |
| 1442 | RAZER CLIO X | 5 | 5 | 0 | 867 | 210 | 190 | 47 |
| 1443 | RAZER LEVIATHAN V2 | 4 | 4 | 0 | 886 | 162 | 246 | 73 |
| 1446 | Razer Seiren V3 Pro | 6 | 6 | 0 | 1522 | 378 | 442 | 7 |
| 1453 | RAZER MAKO X | 6 | 6 | 0 | 1097 | 216 | 249 | 39 |
| 1462 | Razer Kraken Kitty V3 | 5 | 5 | 0 | 928 | 186 | 252 | 82 |
| 1465 | Razer Hammerhead V3 Chroma | 6 | 6 | 0 | 721 | 107 | 172 | 100 |
| 2594 | Razer Turret Mouse Gears Of War 5 Edition | 6 | 6 | 0 | 1655 | 305 | 357 | 8 |
| 2595 | Turret Keyboard Gears Of War 5 Edition | 4 | 4 | 0 | 1163 | 253 | 373 | 9 |
| 2596 | RAZER BLACKWIDOW V3 TENKEYLESS | 3 | 3 | 0 | 1110 | 240 | 357 | 9 |
| 2629 | Razer Wolverine V3 Tournament Edition | 5 | 5 | 0 | 1337 | 239 | 354 | 4 |
| 2636 | Razer Wolverine V3 Pro | 7 | 7 | 0 | 1638 | 312 | 435 | 8 |
| 2638 | Razer BlackShark V3 Pro XBOX | 7 | 7 | 0 | 1714 | 337 | 382 | 155 |
| 2641 | Razer BlackShark V3 XBOX | 7 | 7 | 0 | 1699 | 334 | 382 | 154 |
| 2644 | Razer BlackShark V3 X Hyperspeed | 4 | 4 | 0 | 723 | 139 | 188 | 48 |
| 2647 | Razer Wolverine V3 Pro PC | 6 | 6 | 0 | 1300 | 236 | 350 | 4 |
| 2650 | Razer Wolverine V3 TE PC | 4 | 4 | 0 | 1125 | 191 | 305 | 4 |
| 2657 | Razer Hammerhead V3 HyperSpeed | 5 | 5 | 0 | 883 | 164 | 221 | 66 |
| 2660 | Razer Hammerhead V3 X HyperSpeed | 5 | 5 | 0 | 755 | 126 | 176 | 59 |
| 2664 | Razer Hammerhead V3 X Hyperspeed for Xbox | 5 | 5 | 0 | 755 | 120 | 167 | 60 |
| 2668 | Razer Hammerhead V3 X HyperSpeed For PlayStation | 5 | 5 | 0 | 777 | 126 | 167 | 59 |
| 2676 | Razer Wolverine V4 Pro | 6 | 6 | 0 | 1679 | 325 | 488 | 7 |
| 2684 | Razer Silver T2 X | 5 | 5 | 0 | 1641 | 311 | 478 | 7 |
| 2689 | Razer Hammerhead V3 X HyperSpeed Kuromi Edition | 5 | 5 | 0 | 755 | 126 | 176 | 59 |
| 3072 | Razer Firefly Hard Edition | 2 | 2 | 0 | 562 | 100 | 115 | 4 |
| 3073 | Razer Goliathus Chroma | 2 | 2 | 0 | 563 | 100 | 115 | 4 |
| 3074 | Razer Goliathus Extended Chroma | 2 | 2 | 0 | 562 | 100 | 115 | 4 |
| 3076 | Razer Firefly V2 | 2 | 2 | 0 | 562 | 100 | 115 | 4 |
| 3077 | Razer Strider Chroma | 2 | 2 | 0 | 562 | 100 | 115 | 4 |
| 3078 | Razer Goliathus Chroma 3XL | 2 | 2 | 0 | 562 | 100 | 115 | 4 |
| 3080 | Razer Firefly V2 Pro | 2 | 2 | 0 | 562 | 100 | 115 | 4 |
| 3331 | Razer RipSaw HD | 2 | 2 | 0 | 321 | 60 | 60 | 0 |
| 3334 | Razer Stream Controller | 3 | 3 | 0 | 762 | 137 | 145 | 1 |
| 3337 | Razer Stream Controller X | 3 | 3 | 0 | 762 | 137 | 145 | 1 |
| 3587 | Razer Kiyo | 2 | 2 | 0 | 476 | 88 | 89 | 0 |
| 3589 | Razer Kiyo Pro | 2 | 2 | 0 | 476 | 88 | 89 | 0 |
| 3590 | Razer Kiyo X | 2 | 2 | 0 | 476 | 88 | 89 | 0 |
| 3592 | Razer Kiyo Pro Ultra | 4 | 4 | 0 | 909 | 240 | 227 | 0 |
| 3594 | Razer Kiyo V2 Pro | 4 | 4 | 0 | 1015 | 252 | 273 | 0 |
| 3595 | Razer Kiyo V2 | 5 | 5 | 0 | 1039 | 260 | 278 | 0 |
| 3596 | Razer Kiyo V2 X | 4 | 4 | 0 | 886 | 226 | 243 | 0 |
| 3848 | RAZER BASE STATION CHROMA | 2 | 2 | 0 | 562 | 100 | 115 | 4 |
| 3849 | Razer Chroma HDK | 2 | 2 | 0 | 534 | 96 | 101 | 4 |
| 3853 | Razer Laptop Stand Chroma | 2 | 2 | 0 | 562 | 100 | 115 | 4 |
| 3858 | RAZER RAPTOR 27 | 5 | 5 | 0 | 968 | 190 | 176 | 4 |
| 3859 | Lian Li 011 Dynamic | 2 | 2 | 0 | 562 | 100 | 115 | 4 |
| 3863 | Razer Tomahawk ATX | 2 | 2 | 0 | 562 | 100 | 115 | 4 |
| 3866 | Razer Core X Chroma | 2 | 2 | 0 | 566 | 102 | 115 | 4 |
| 3867 | Razer Seiren Emote | 3 | 3 | 0 | 384 | 70 | 74 | 0 |
| 3869 | Razer Bungee V3 Chroma | 2 | 2 | 0 | 532 | 94 | 111 | 4 |
| 3871 | Razer Chroma Addressable RGB Controller | 3 | 3 | 0 | 798 | 138 | 153 | 6 |
| 3872 | Razer Base Station V2 Chroma | 3 | 3 | 0 | 678 | 128 | 140 | 4 |
| 3873 | Razer Thunderbolt 4 Dock Chroma | 3 | 3 | 0 | 666 | 125 | 138 | 4 |
| 3878 | Razer Chroma Charging Pad 10W Fast Wireless Charger | 2 | 2 | 0 | 605 | 117 | 133 | 1 |
| 3879 | Tomahawk Gaming Desktop | 2 | 2 | 0 | 562 | 100 | 115 | 4 |
| 3880 | RAZER RAPTOR 27 (165Hz) | 5 | 5 | 0 | 1044 | 205 | 191 | 4 |
| 3883 | Razer Laptop Stand V2 Chroma | 2 | 2 | 0 | 562 | 100 | 115 | 4 |
| 3884 | Razer Chroma Wireless ARGB Controller | 3 | 3 | 0 | 823 | 143 | 155 | 6 |
| 3886 | Razer Chroma Wireless ARGB Controller | 5 | 5 | 0 | 927 | 133 | 188 | 12 |
| 3893 | Razer Hanbo AIO 240MM ARGB | 3 | 3 | 0 | 723 | 125 | 138 | 6 |
| 3894 | Razer Head Cushion Chroma | 2 | 2 | 0 | 562 | 100 | 115 | 4 |
| 3900 | RAZER PWM PC FAN CONTROLLER | 2 | 2 | 0 | 456 | 76 | 78 | 0 |
| 3907 | Razer Laptop Cooling Pad | 3 | 3 | 0 | 1276 | 223 | 334 | 8 |
| 3909 | Razer Freyja | 2 | 2 | 0 | 619 | 113 | 127 | 4 |
| 3921 | Razer Core X V2 | 3 | 3 | 0 | 569 | 129 | 173 | 1 |
| 3922 | Razer Thunderbolt 5 Dock Chroma | 3 | 3 | 0 | 670 | 126 | 146 | 27 |
| 3929 | Razer Monitor Stand Chroma | 2 | 2 | 0 | 935 | 171 | 256 | 8 |
| 3932 | Handheld Gaming Dock | 2 | 2 | 0 | 936 | 173 | 255 | 8 |
| 3940 | Razer Soma Chroma | 3 | 3 | 0 | 606 | 112 | 128 | 26 |
| 3942 | Razer Marci | 7 | 7 | 0 | 1795 | 384 | 416 | 133 |
| 3946 | August T2 | 3 | 3 | 0 | 1897 | 385 | 539 | 14 |
| 3949 | MarciT2H | 3 | 3 | 0 | 657 | 123 | 140 | 4 |
| 4115 | Razer Kitsune | 3 | 3 | 0 | 655 | 123 | 139 | 4 |
| 4124 | Razer BlackShark V3 Pro PS | 7 | 7 | 0 | 1714 | 337 | 382 | 155 |
| 4126 | Razer BlackShark V3 PS | 7 | 7 | 0 | 1699 | 334 | 382 | 154 |
| 4130 | Razer BlackShark V3 X Hyperspeed | 4 | 4 | 0 | 723 | 139 | 188 | 48 |
| 4133 | Razer Raiju V3 Pro | 6 | 6 | 0 | 1331 | 241 | 351 | 4 |
| 4144 | Razer Raiju V3 Pro Signature Edition | 6 | 6 | 0 | 1333 | 241 | 361 | 4 |
| 45066 | Razer Basilisk V3 Pro 35K - XBOX 25 Anniversary Edition | 6 | 6 | 0 | 1055 | 211 | 222 | 6 |

完整逐页证据在 JSON/gzip；该清单是逆向工作队列和可回查索引，不是当前 Rust UI 已达到原版的声明。继续闭环时，应按 page_id 读取根 source，再跟随组件 props、imports、事件 callback、host/service 调用及 DLL 静态 wrapper。
