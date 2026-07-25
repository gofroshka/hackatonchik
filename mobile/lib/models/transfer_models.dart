enum SonicMode { send, receive, chat }

enum TransferPhase {
  idle,
  preparing,
  handshaking,
  handshakeWaitAck,
  sending,
  listening,
  endWaitAck,
  completed,
  error,
}

enum ChatMessageState { sending, sent, failed }

class ChatMessage {
  const ChatMessage({
    required this.id,
    required this.text,
    required this.outgoing,
    required this.createdAt,
    required this.state,
  });

  final String id;
  final String text;
  final bool outgoing;
  final DateTime createdAt;
  final ChatMessageState state;

  ChatMessage withState(ChatMessageState value) => ChatMessage(
    id: id,
    text: text,
    outgoing: outgoing,
    createdAt: createdAt,
    state: value,
  );
}

class ReceivedTransfer {
  const ReceivedTransfer({
    required this.name,
    required this.contentType,
    required this.path,
    required this.size,
    this.text,
  });

  final String name;
  final String contentType;
  final String path;
  final int size;
  final String? text;
}
