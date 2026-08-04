import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';

void main() => runApp(const WseApp());

/// One workspace as reported by the server.
class Workspace {
  final String id, name, state;
  final int apps;
  Workspace(this.id, this.name, this.state, this.apps);
}

/// Talks to the local wse-server (127.0.0.1:47611): sends text commands, receives
/// one JSON state line per reply. Auto-reconnects if the server isn't up yet.
class WseClient extends ChangeNotifier {
  Socket? _socket;
  List<Workspace> workspaces = [];
  bool connected = false;

  WseClient() {
    _connect();
  }

  Future<void> _connect() async {
    try {
      _socket = await Socket.connect('127.0.0.1', 47611);
      connected = true;
      notifyListeners();
      _socket!
          .cast<List<int>>()
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .listen(_onLine, onDone: _reconnect, onError: (_) => _reconnect());
    } catch (_) {
      _reconnect();
    }
  }

  void _reconnect() {
    connected = false;
    _socket = null;
    notifyListeners();
    Future.delayed(const Duration(seconds: 1), _connect);
  }

  void _onLine(String line) {
    try {
      final data = jsonDecode(line) as Map<String, dynamic>;
      workspaces = (data['workspaces'] as List)
          .map((w) => Workspace(w['id'], w['name'], w['state'], w['apps']))
          .toList();
      notifyListeners();
    } catch (_) {}
  }

  void send(String cmd) => _socket?.write('$cmd\n');
}

class WseApp extends StatelessWidget {
  const WseApp({super.key});
  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'WSE Desktop',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        useMaterial3: true,
        brightness: Brightness.dark,
        colorSchemeSeed: const Color(0xFF6366F1),
        scaffoldBackgroundColor: const Color(0xFF14151C),
        fontFamily: 'Segoe UI',
      ),
      home: const HomePage(),
    );
  }
}

class HomePage extends StatefulWidget {
  const HomePage({super.key});
  @override
  State<HomePage> createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  final client = WseClient();
  final nameCtrl = TextEditingController();

  @override
  void initState() {
    super.initState();
    client.addListener(_update);
  }

  void _update() => setState(() {});

  @override
  void dispose() {
    client.removeListener(_update);
    nameCtrl.dispose();
    super.dispose();
  }

  void _create() {
    final name = nameCtrl.text.trim();
    client.send(name.isEmpty ? 'create Workspace' : 'create $name');
    nameCtrl.clear();
  }

  Color _stateColor(String s) => switch (s) {
        'running' => const Color(0xFF4ADE80),
        'suspended' => const Color(0xFFFACC15),
        _ => const Color(0xFF8B8FA3),
      };

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(28, 24, 28, 24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  const Text('WSE Desktop',
                      style: TextStyle(fontSize: 26, fontWeight: FontWeight.w700)),
                  const SizedBox(width: 12),
                  _ConnDot(connected: client.connected),
                ],
              ),
              const SizedBox(height: 4),
              Text('your workspaces',
                  style: TextStyle(color: Colors.white.withValues(alpha: 0.5))),
              const SizedBox(height: 20),
              _createRow(),
              const SizedBox(height: 20),
              Expanded(
                child: client.workspaces.isEmpty
                    ? _empty()
                    : ListView.separated(
                        itemCount: client.workspaces.length,
                        separatorBuilder: (_, __) => const SizedBox(height: 12),
                        itemBuilder: (_, i) => _card(client.workspaces[i]),
                      ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _createRow() {
    return Row(
      children: [
        Expanded(
          child: TextField(
            controller: nameCtrl,
            onSubmitted: (_) => _create(),
            decoration: InputDecoration(
              hintText: 'New workspace name…',
              filled: true,
              fillColor: const Color(0xFF23242E),
              border: OutlineInputBorder(
                borderRadius: BorderRadius.circular(12),
                borderSide: BorderSide.none,
              ),
              contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
            ),
          ),
        ),
        const SizedBox(width: 12),
        FilledButton(
          onPressed: client.connected ? _create : null,
          style: FilledButton.styleFrom(
            padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 18),
            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
          ),
          child: const Text('Create'),
        ),
      ],
    );
  }

  Widget _empty() {
    return Center(
      child: Text(
        client.connected
            ? 'No workspaces yet — create one above.'
            : 'Connecting to WSE server…\nStart wse-server.exe.',
        textAlign: TextAlign.center,
        style: TextStyle(color: Colors.white.withValues(alpha: 0.5)),
      ),
    );
  }

  Widget _card(Workspace w) {
    return Container(
      padding: const EdgeInsets.fromLTRB(20, 16, 16, 16),
      decoration: BoxDecoration(
        color: const Color(0xFF20222C),
        borderRadius: BorderRadius.circular(16),
      ),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(w.name,
                    style: const TextStyle(fontSize: 17, fontWeight: FontWeight.w600)),
                const SizedBox(height: 6),
                Row(
                  children: [
                    Container(
                      width: 8,
                      height: 8,
                      decoration: BoxDecoration(
                          color: _stateColor(w.state), shape: BoxShape.circle),
                    ),
                    const SizedBox(width: 8),
                    Text('${w.state}   ·   ${w.apps} app(s)',
                        style: TextStyle(color: Colors.white.withValues(alpha: 0.55))),
                  ],
                ),
              ],
            ),
          ),
          _action('Enter', Icons.login, () => client.send('enter ${w.id}'), filled: true),
          const SizedBox(width: 8),
          _action('Launch', Icons.public, () => client.send('launch ${w.id}')),
          const SizedBox(width: 8),
          _action('Suspend', Icons.pause, () => client.send('suspend ${w.id}')),
          const SizedBox(width: 8),
          _action('Destroy', Icons.delete_outline, () => client.send('destroy ${w.id}'),
              danger: true),
        ],
      ),
    );
  }

  Widget _action(String label, IconData icon, VoidCallback onTap,
      {bool filled = false, bool danger = false}) {
    final fg = danger ? const Color(0xFFFF6B7A) : null;
    final child = Tooltip(
      message: label,
      child: Icon(icon, size: 20, color: fg),
    );
    return filled
        ? IconButton.filled(onPressed: onTap, icon: child)
        : IconButton(onPressed: onTap, icon: child);
  }
}

class _ConnDot extends StatelessWidget {
  final bool connected;
  const _ConnDot({required this.connected});
  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
      decoration: BoxDecoration(
        color: (connected ? const Color(0xFF4ADE80) : const Color(0xFFFF6B7A))
            .withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(20),
      ),
      child: Text(
        connected ? 'connected' : 'offline',
        style: TextStyle(
          fontSize: 12,
          color: connected ? const Color(0xFF4ADE80) : const Color(0xFFFF6B7A),
        ),
      ),
    );
  }
}
