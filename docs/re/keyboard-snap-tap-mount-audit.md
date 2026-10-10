# Current Snap Tap component candidates and caller audit

Static bounded scan of current official product bundles. Vendor JavaScript was read as bytes and never executed; no docs snapshot was used. `snap-tap-widget` is a shared component marker; its presence alone does not prove an actual mount. Each candidate row records exact file, byte offset, byte length and SHA-256. Only PID 515, 565, 567, 585, 659, 716 and 752 have complete component/editor/row/CSS and caller receipts in `keyboard-snap-tap-pages/` and may enable the ordinary Rust component. The remaining rows require full caller/branch review and are not enabled by this candidate table.

| PID | Source bundle | Byte offset | Bytes | SHA-256 |
|---:|---|---:|---:|---|
| 515 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/js/main.f60ca5aa.js` | 8143643 | 8376694 | `4a9db2072b3d64035e36d468fcc006b1930ebbf8c0bc620e4869a2e970c432e8` |
| 521 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/521/ui/static/js/main.bbf02831.js` | 7789856 | 8022462 | `47661189de4f42754c0e78c37c2fb55c1ff8cb133629e3f90129042443bc8057` |
| 529 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/529/ui/static/js/main.c4a3f376.js` | 7675589 | 7908836 | `c571f7c16424069aa8f06ac51a0e06bf0ce0fa22db2c22963324906f47c30d09` |
| 534 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/534/ui/static/js/main.0cc999dc.js` | 7872686 | 8105936 | `620a0279e6acddb6d63b0b88d10601e754a7ca009f2c1e401bb237597892157c` |
| 542 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/542/ui/static/js/main.1e52c858.js` | 8187207 | 8425059 | `574ff3a933c6d9eb6049bf0e35a1863516ea724e975c0f82fd7528b60fead583` |
| 545 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/545/ui/static/js/main.c8080dd3.js` | 8305914 | 8552706 | `550d7a32da55526995d0f155750a2face3fe8210afac27820be2cd0fdaada79f` |
| 550 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/550/ui/static/js/4802.699ed88f.chunk.js` | 707286 | 1912420 | `5ce2179ea3347b7e99ba970b5173d4dad1811bafae8bcc1db837051e3eb6daab` |
| 551 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/551/ui/static/js/main.5f9f6113.js` | 8001408 | 8096403 | `657421b24601e652dbaaea5a43ff081318857a6c23b93500a244d74499ade87f` |
| 552 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/552/ui/static/js/main.bdf1be1e.js` | 7990043 | 8237993 | `f75db54781879f8e507f2a862bcb77e4ce0fb30a64e2cd1d772efa5d732579a4` |
| 554 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/554/ui/static/js/main.faed76d3.js` | 8167625 | 8400418 | `f396ad97c8c303e9f3c947c95dd09e6cb754ad3fa48d5ddb1356b1565ef89eac` |
| 555 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/555/ui/static/js/main.110aac54.js` | 7450730 | 7555986 | `c92ffbce91f24d8cc93fb61de959a6b9a033eaa67fa0baceb841650074067611` |
| 556 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/556/ui/static/js/main.3c239c1f.js` | 7371660 | 7604278 | `b29dc471b134da9e1f865d6721ffef8d33744360ada23767813676b1f9580c60` |
| 565 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/js/main.3280575a.js` | 7467151 | 7699375 | `02384c503b5b673215daf6434c0d6bcd2392f8bd38d7ebc84c0250ec99786391` |
| 567 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/js/main.33124f31.js` | 8191082 | 8425605 | `8bfe6cc4ee10ce0b01c86063a46bfa25710664fcab22eabdc405396890b39b3a` |
| 574 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/574/ui/static/js/main.4e1cb347.js` | 7460427 | 7661130 | `a9b14ed0e9253d0dbce7189144db145eea1d68140872a0acbc8a1c3bd11d4d11` |
| 575 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/575/ui/static/js/main.e6809e11.js` | 8102874 | 8336296 | `3426e8fc3dba68241305a8973d8a3b5806b6f819766fa47d28ab8da3231fdc6e` |
| 577 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/577/ui/static/js/main.7053d506.js` | 8263880 | 8510663 | `0c59b3c036c55c3fa153b4e78d9cd7b1310e90edf42984b346344a5334ac35fd` |
| 579 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/579/ui/static/js/main.3355e99f.js` | 7913374 | 8008746 | `8856a23f61053e2721929901e31caf218b8a17c1adc24fab2a5749f5dc17cfd0` |
| 585 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/585/ui/static/js/main.04a0e3c0.js` | 7402791 | 7650942 | `1f3366afcee6de7d59812a3ec1ea5e1060bc5b27c402fda694ee6ab6659d0bb3` |
| 590 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/590/ui/static/js/main.5c506335.js` | 8150269 | 8649125 | `f31b865f40c73eeca253dd23d63fb0b212131ac36ab68b380f27899ebf202c0f` |
| 591 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/591/ui/static/js/main.60b1f83e.js` | 7359210 | 7604304 | `d4ba833954d8fc113ad6724520ec910d301a30469aa4c912a62ab062bbe42db0` |
| 592 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/592/ui/static/js/main.319b06ba.js` | 7549509 | 7646066 | `4f006da3c62a1b7eb4a63c8849484218abc3e14ff449d6d356ff0f8312192463` |
| 593 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/593/ui/static/js/main.5ba3a935.js` | 8107194 | 8339593 | `3a6ef092fa9e73956c76ddfa8ca356c4e249023489897c7d3d9fc6df821638ac` |
| 599 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/599/ui/static/js/main.7e39cfe6.js` | 7791357 | 8026235 | `78fcee257ee6fc275b0d899c123a72746c1652d7262d27d7b856ba1ca8f00b64` |
| 600 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/600/ui/static/js/main.d94b34ab.js` | 7770914 | 8006677 | `60fb5b159802507559d74a22c62675ce09be2bdac14dbc81fa7e50a71365f674` |
| 602 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/602/ui/static/js/main.79e7fc3d.js` | 8252632 | 8501584 | `9287537e3781602612f596054ae1e9b3e65af62ce8d6a4838950c20718133213` |
| 605 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/605/ui/static/js/main.fcf07de2.js` | 8369115 | 8603286 | `307e2b199037ed53a8fde90d76fc67a9a3766a9361fb152e7820d8bf05f19c65` |
| 606 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/606/ui/static/js/main.08f3ff4a.js` | 8197046 | 8444897 | `682c63cbb46ec427922aee93d181025430eea1e4ddb86a43ef419561d7fda4ab` |
| 614 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/614/ui/static/js/main.73d0babc.js` | 8202646 | 8276539 | `7f1d1fa7dcacc88b2feb987b58cfcafcab43d545fe6fd8f7bd071a09b99c27d0` |
| 617 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/617/ui/static/js/main.bcdaed34.js` | 7452423 | 7699631 | `7210f9879cf44fcb00fcd052b834dbca6a6c357f247811e6f4909da859432de0` |
| 619 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/619/ui/static/js/main.f420bb87.js` | 7885513 | 7982219 | `80d032fe2308a6ed6479cb91254f72e35a2c08e682e92036fbb6bcb5fa7242c3` |
| 620 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/620/ui/static/js/main.05ff3291.js` | 8132127 | 8230716 | `cf915530c7cc522d18e564ad0b39ebbd09bdc780b01fdbbb01c5be3912965f7e` |
| 631 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/631/ui/static/js/main.9463ff90.js` | 7787085 | 8037736 | `816c853bc52d826288c3938507a7a74b833f62295ac2d4d5663ee5ea489785e1` |
| 642 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/642/ui/static/js/main.c81e7939.js` | 7988379 | 8053235 | `ab41c1cd82ccc8350500d48216a214f689bee9836cfa5b2db6a34aeb898889ae` |
| 647 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/647/ui/static/js/main.b14e4864.js` | 8172637 | 8670119 | `eb33cc70689435a4be775d5891b0079c1099bfebde3021ff0f8e2ccc88c5a9e9` |
| 654 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/654/ui/static/js/main.e100f1a2.js` | 7271989 | 7502723 | `39c64eb240d68a73773952bfa58712a72d2dfdd9159c16964dfe322bf3f00c01` |
| 655 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/655/ui/static/js/main.f6ad51bc.js` | 8465884 | 8703300 | `6c8f5b3ad74b9793bb2fa83d4d82b0b108767b80bec41bd176f8455251e9f734` |
| 658 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/658/ui/static/js/main.62faa864.js` | 8240945 | 8478674 | `e1864fc45ad57b626882fd3862e1ed16b41a69632685d957b5e6561134d7ae70` |
| 659 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/js/main.82fcdd62.js` | 8180354 | 8677813 | `f71ec3026586ab2968bd719dcca140db55edb86a82c9adc0f1a00a6495e1ae8c` |
| 660 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/660/ui/static/js/main.85645113.js` | 8294404 | 8527233 | `0ea92ddb4a3c0935afc3de0e95f67a44b38b900b2e69685eb649ad1f598e7acd` |
| 661 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/661/ui/static/js/main.db42d2c2.js` | 8420677 | 8620890 | `4a398c01f1f2279f2307382f24e9c566b32c233e85d64ad8f907d5cc842ece6c` |
| 664 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/664/ui/static/js/main.33d8ba2a.js` | 8117303 | 8370459 | `3397f2e4e8ac4d959c21e6f7bf573b11fadbb45e052bb66ee0a182d58eaee5ab` |
| 673 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/673/ui/static/js/main.7c592b5e.js` | 8533291 | 8767388 | `bbdaf4b8d128322b6f82c7de8c36d137c7dff5b2f51560b283666ecc073c87fb` |
| 674 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/674/ui/static/js/main.b8c0b92d.js` | 8417526 | 8650221 | `34257871b658776df847e7e3a7d275452dc9c572cdffbc820480c21b6546aadd` |
| 675 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/675/ui/static/js/main.b4dbb43b.js` | 8180291 | 8420350 | `62fa9afec69486f5a1fb043f37a7f71bd1695577b0f41b4c9cd21411e8fed845` |
| 677 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/677/ui/static/js/main.b1c9555f.js` | 7600140 | 7856718 | `1be1d1322afcdcc8f5f8ec7835af6fa80476c98d081a833c885953044e1f050e` |
| 689 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/689/ui/static/js/main.9d55a71f.js` | 7818633 | 8051638 | `525e6cf579c988539c496cef57583659f4cd47f77d8fcd71113c3cd59b598f56` |
| 691 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/691/ui/static/js/6375.a5fed9ed.chunk.js` | 767565 | 1970967 | `a44c6bb18a16d2ff652f19a024f88f9f65a54b4da165e2deaff62a652044aa10` |
| 697 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/697/ui/static/js/main.9db50ae0.js` | 7766039 | 8002283 | `575eefa164cb9555083e91a29ccdfa588cc55f2c275ebfa1651591fb8e56d502` |
| 708 | **excluded: no ordinary main-bundle marker; route chunk requires separate audit** | ? | ? | ? |
| 716 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/js/main.a149c89a.js` | 7870148 | 8129950 | `c4d563c8ed9d05de9268905fad8e600a7bd8710c3340fe774d78f980fc7d3de7` |
| 717 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/717/ui/static/js/main.9af2005e.js` | 7593439 | 7856919 | `4222c13514e4d7c295e18057a5100b188f036b8825e10cb7442174cf2c359a5b` |
| 724 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/724/ui/static/js/main.4b1b7b70.js` | 7989580 | 8256875 | `b36e0ccc6ddfc312beac25b124fc5ed9037f3fb29e838b4830c13a5f58b57d01` |
| 752 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/js/8141.480d3f0c.chunk.js` | 1740269 | 2009248 | `bca8156dab8bb0d015e56d220d87f901bb070e0ae287a14697392eef6f8df66d` |
| 727 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/727/ui/static/js/main.07c5d653.js` | 7613553 | 7884721 | `ef8c412b51386d4f70e5485365120111c30ab0ce147756dc64ffade1c74fef71` |
| 2595 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/2595/ui/static/js/main.01d8ae5b.js` | 7461496 | 7662199 | `8194032e6c8c7cae8d1ed09cc110491f44cc67a841dc0a5a1a7140e10586ea1b` |
| 2596 | `local-ui-reverse/source/official/apps.razer.com/synapse/products/2596/ui/static/js/main.eff25ed8.js` | 8039027 | 8272978 | `16e7b6c5859c316c8212cccf484dcb1c0699bab162d9bc23cbf583ca74a5f41e` |

## Source conditions

The ordinary component reads `snapTapReducer.isEnabled` and `keyList`, blocks `KEY_APPLICATION`, `KEY_LEFT_GUI`, `KEY_FN`, `DKM_F6`, and `DKM_D2`, rejects duplicate values, captures `keyboard`, `analogKey`, and `razerKey` input redirects, and mounts only within the Customize page. The source toggle also checks adjustment mode and publishes the system write request with `enabled`, `snapTapPairs`, and the localized label. Products 614 and 642 (analog), plus v3/v4 or dual-key variants, have separate source components and are intentionally excluded from the ordinary Rust component until their rows and state machine are separately implemented.

The Rust implementation currently has an explicit `ordinary_product` allowlist and still lacks the native middleware write-back/observation adapter. Local draft persistence must not be presented as device save.
