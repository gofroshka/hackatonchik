import 'dart:typed_data';

/// A fixed-capacity circular buffer of doubles used to hold recent PCM samples
/// for the streaming decoder.
///
/// When the buffer is full, the oldest samples are overwritten. The caller can
/// snapshot the current contents in chronological order via [toList].
class RingBuffer {
  RingBuffer(this.capacity) : _data = Float64List(capacity);

  final int capacity;
  final Float64List _data;

  int _start = 0;
  int _length = 0;

  /// Number of valid samples currently stored.
  int get length => _length;

  bool get isFull => _length == capacity;

  /// Appends [samples]; oldest samples are dropped if capacity is exceeded.
  void addAll(List<double> samples) {
    for (final s in samples) {
      add(s);
    }
  }

  /// Appends a single sample.
  void add(double sample) {
    final writeIndex = (_start + _length) % capacity;
    _data[writeIndex] = sample;
    if (_length < capacity) {
      _length++;
    } else {
      _start = (_start + 1) % capacity;
    }
  }

  /// Removes the oldest [count] samples.
  void drop(int count) {
    final n = count.clamp(0, _length);
    _start = (_start + n) % capacity;
    _length -= n;
  }

  /// Returns a chronological copy of the current contents.
  Float64List toList() {
    final out = Float64List(_length);
    for (int i = 0; i < _length; i++) {
      out[i] = _data[(_start + i) % capacity];
    }
    return out;
  }

  void clear() {
    _start = 0;
    _length = 0;
  }
}
