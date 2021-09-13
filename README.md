# reversed-word-byte-rw

Implementation of `Read + Write + Seek` for data which is stored as opposite endian words.

For example, data which is logically `[0, 1, 2, 3, 4, 5, 6, 7]` but is actually stored as `[3, 2, 1, 0, 7, 6, 5, 4]`.

This crate allows for reading and writing from arbitrary bytes in a slice without having to consider how the underlying data is stored and aligning to word boundaries.