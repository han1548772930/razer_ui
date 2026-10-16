`current-game-list.xml` contains only synthetic records. Its `.enc` counterpart
was prepared with the system .NET AES implementation using CBC/PKCS7 and the
current GMS3 local file-format key and IV. Tests decode this independent
ciphertext with Rust AES and compare every matching field with the plain XML.
Neither fixture contains installed catalogs, user accounts, or process data.
