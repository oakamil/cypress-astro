// Copyright (c) 2026 Omair Kamil
//
// This file is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, version 3 of the License.
//
// This file is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

import 'package:flutter/material.dart';
import 'package:cedar_flutter/client_main.dart';
import 'package:http/http.dart' as http;
import 'package:file_selector/file_selector.dart';
import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:cedar_flutter/platform.dart';
import 'package:cedar_flutter/settings.dart';
import 'dart:io';

void showUpdaterInfoDialog(MyHomePageState state, BuildContext context,
    {bool filterUpdateFiles = false}) {
  showDialog(
    context: context,
    builder: (context) => UpdaterInfoDialog(state: state),
  );
}

class UpdaterInfoDialog extends StatefulWidget {
  final MyHomePageState state;
  const UpdaterInfoDialog({Key? key, required this.state}) : super(key: key);

  @override
  _UpdaterInfoDialogState createState() => _UpdaterInfoDialogState();
}

class _UpdaterInfoDialogState extends State<UpdaterInfoDialog> {
  bool _isUploading = false;
  bool _isDone = false;
  String? _errorMessage;

  Future<void> _uploadUpdate() async {
    try {
      final XFile? result = await openFile();

      if (result != null) {
        widget.state.updateInProgress = true;
        setState(() {
          _isUploading = true;
          _errorMessage = null; // Clear previous errors
        });

        final bytes = await result.readAsBytes();

        final host = kIsWeb ? Uri.base.host : await resolveCedarHost();
        final postUri = Uri.parse("http://$host:8081/update-system");

        final postResponse = await http.post(postUri, body: bytes);

        if (postResponse.statusCode == 200) {
          if (mounted) {
            setState(() {
              _isDone = true;
            });
          }
        } else {
          debugPrint(
              "Failed to upload update data: ${postResponse.statusCode}");
          if (mounted) {
            setState(() {
              _errorMessage = "Validation Failed: ${postResponse.body}";
            });
          }
        }
      }
    } catch (e) {
      debugPrint("Error uploading update: $e");
      if (mounted) {
        setState(() {
          _errorMessage = "Upload Error: $e";
        });
      }
    } finally {
      widget.state.updateInProgress = false;
      if (mounted) {
        setState(() {
          _isUploading = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final color = Theme.of(context).colorScheme.primary;

    return DefaultTextStyle.merge(
      style: const TextStyle(fontFamilyFallback: ['Roboto']),
      child: Center(
        child: Material(
          color: Colors.transparent,
          child: Container(
            width: 210.0 * textScaleFactor(context),
            padding: const EdgeInsets.fromLTRB(10, 5, 10, 10),
            decoration: BoxDecoration(
              border: Border.all(color: color),
              color: Colors.black,
              borderRadius: BorderRadius.circular(10),
            ),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Row(mainAxisAlignment: MainAxisAlignment.center, children: [
                  Text("System Update",
                      style:
                          TextStyle(color: color, fontWeight: FontWeight.bold),
                      textScaler: textScaler(context)),
                ]),
                const SizedBox(height: 15),
                if (_errorMessage != null) ...[
                  Text(
                    _errorMessage!,
                    style: const TextStyle(color: Colors.redAccent),
                    textAlign: TextAlign.center,
                  ),
                  const SizedBox(height: 15),
                  TextButton.icon(
                    icon: const Icon(Icons.refresh),
                    label: Text("Try Again",
                        style: TextStyle(color: color),
                        textScaler: textScaler(context)),
                    onPressed: () {
                      setState(() {
                        _errorMessage = null;
                      });
                    },
                  ),
                ] else if (_isDone) ...[
                  Text(
                    "The update will be applied on the next restart.",
                    style: TextStyle(color: color),
                    textAlign: TextAlign.center,
                  ),
                  const SizedBox(height: 15),
                  TextButton.icon(
                    icon: const Icon(Icons.restart_alt),
                    label: Text("Restart Now",
                        style: TextStyle(color: color),
                        textScaler: textScaler(context)),
                    onPressed: () {
                      widget.state.restart();
                      Navigator.of(context).pop();
                    },
                  ),
                ] else if (_isUploading) ...[
                  CircularProgressIndicator(color: color),
                  const SizedBox(height: 10),
                  Text("Uploading...", style: TextStyle(color: color)),
                ] else ...[
                  TextButton.icon(
                    icon: const Icon(Icons.file_upload),
                    label: Text("Upload",
                        style: TextStyle(color: color),
                        textScaler: textScaler(context)),
                    onPressed: _uploadUpdate,
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}
