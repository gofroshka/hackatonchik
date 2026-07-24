/// A tiny growable builder for bit sequences.
class BitStreamBuilder {
  final List<int> _bits = <int>[];

  /// The accumulated bits.
  List<int> get bits => _bits;

  /// Appends a single bit (masked to 0/1).
  void addBit(int bit) => _bits.add(bit & 1);

  /// Appends a list of bits.
  void addBits(Iterable<int> bits) {
    for (final b in bits) {
      _bits.add(b & 1);
    }
  }
}
