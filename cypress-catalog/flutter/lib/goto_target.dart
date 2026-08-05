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
import 'package:cedar_flutter/cedar.pb.dart' as cedar_pb;
import 'package:cedar_flutter/cedar_common.pb.dart' as cedar_common;
import 'package:cedar_flutter/settings.dart';

/// Wired up in client_main.dart
void gotoRaDecDialog(MyHomePageState state, BuildContext context) {
  showDialog(
    context: context,
    builder: (context) => GotoDialog(state: state),
  );
}

class GotoDialog extends StatefulWidget {
  final MyHomePageState state;
  const GotoDialog({Key? key, required this.state}) : super(key: key);

  @override
  _GotoDialogState createState() => _GotoDialogState();
}

class _GotoDialogState extends State<GotoDialog> {
  String _epoch = 'J2000';

  // RA Selection State
  int _raH = 0;
  int _raM = 0;
  int _raS = 0;

  // Dec Selection State
  String _decSign = '+';
  int _decD = 0;
  int _decM = 0;
  int _decS = 0;

  double _calculateEpochNow() {
    final now = DateTime.now();
    final isLeapYear =
        (now.year % 4 == 0 && now.year % 100 != 0) || (now.year % 400 == 0);
    final daysInYear = isLeapYear ? 366 : 365;

    // Calculate day of the year
    final firstDayOfYear = DateTime(now.year, 1, 1);
    final dayOfYear = now.difference(firstDayOfYear).inDays + 1;

    final epochDecimal = now.year + (dayOfYear / daysInYear);
    // Round to the nearest tenth digit
    return (epochDecimal * 10).roundToDouble() / 10.0;
  }

  void _submitGoto() {
    // Convert RA to degrees: (hours + minutes/60 + seconds/3600) * 15
    final raDecimal = (_raH + (_raM / 60.0) + (_raS / 3600.0)) * 15.0;

    // Convert Dec to degrees
    double decDecimal = _decD + (_decM / 60.0) + (_decS / 3600.0);
    if (_decSign == '-') {
      decDecimal = -decDecimal;
    }

    final coord = cedar_common.CelestialCoord()
      ..ra = raDecimal
      ..dec = decDecimal;

    // If "J2000" is selected, Epoch remains unpopulated.
    if (_epoch == 'Now') {
      coord.epoch = _calculateEpochNow();
    }

    final req = cedar_pb.ActionRequest()..initiateSlew = coord;
    widget.state.initiateAction(req);
    Navigator.of(context).pop();
  }

  List<DropdownMenuItem<int>> _buildIntItems(
      int min, int max, Color color, BuildContext context) {
    return List.generate(max - min + 1, (index) {
      final val = min + index;
      return DropdownMenuItem<int>(
        value: val,
        child: Text('$val',
            style: TextStyle(color: color), textScaler: textScaler(context)),
      );
    });
  }

  Widget _buildDropdownWithSuffix<T>({
    required T value,
    required List<DropdownMenuItem<T>> items,
    required void Function(T?)? onChanged,
    required Color color,
    required BuildContext context,
    String? suffix,
  }) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        DropdownButton<T>(
          value: value,
          dropdownColor: Colors.black,
          style: TextStyle(color: color),
          iconEnabledColor: color,
          iconDisabledColor: color.withAlpha(100),
          underline: Container(
            height: 1,
            color: onChanged == null ? Colors.transparent : color,
          ),
          items: items,
          onChanged: onChanged,
        ),
        if (suffix != null)
          Padding(
            padding: const EdgeInsets.only(left: 4.0),
            child: Text(
              suffix,
              style: TextStyle(color: color),
              textScaler: textScaler(context),
            ),
          ),
      ],
    );
  }

  @override
  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final color = theme.colorScheme.primary;

    return Theme(
      data: theme.copyWith(
        scrollbarTheme: ScrollbarThemeData(
          thumbColor: WidgetStateProperty.all(color),
          trackColor: WidgetStateProperty.all(color.withAlpha(50)),
        ),
      ),
      child: DefaultTextStyle.merge(
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
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Row(mainAxisAlignment: MainAxisAlignment.center, children: [
                    Text("Enter Target",
                        style: TextStyle(
                            color: color, fontWeight: FontWeight.bold),
                        textScaler: textScaler(context)),
                  ]),
                  const SizedBox(height: 5),

                  // Epoch Selection
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text("Epoch",
                          style: TextStyle(color: color),
                          textScaler: textScaler(context)),
                      DropdownButton<String>(
                        value: _epoch,
                        dropdownColor: Colors.black,
                        style: TextStyle(color: color),
                        iconEnabledColor: color,
                        underline: Container(height: 1, color: color),
                        items: ['J2000', 'Now']
                            .map((e) => DropdownMenuItem(
                                  value: e,
                                  child: Text(e,
                                      style: TextStyle(color: color),
                                      textScaler: textScaler(context)),
                                ))
                            .toList(),
                        onChanged: (val) => setState(() => _epoch = val!),
                      ),
                    ],
                  ),

                  // RA Selection
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text("Right ascension",
                          style: TextStyle(color: color),
                          textScaler: textScaler(context)),
                    ],
                  ),
                  Wrap(
                    spacing: 4.0,
                    children: [
                      _buildDropdownWithSuffix<int>(
                        value: _raH,
                        color: color,
                        context: context,
                        items: _buildIntItems(0, 23, color, context),
                        onChanged: (val) => setState(() => _raH = val!),
                        suffix: 'h ',
                      ),
                      _buildDropdownWithSuffix<int>(
                        value: _raM,
                        color: color,
                        context: context,
                        items: _buildIntItems(0, 59, color, context),
                        onChanged: (val) => setState(() => _raM = val!),
                        suffix: 'm ',
                      ),
                      _buildDropdownWithSuffix<int>(
                        value: _raS,
                        color: color,
                        context: context,
                        items: _buildIntItems(0, 59, color, context),
                        onChanged: (val) => setState(() => _raS = val!),
                        suffix: 's ',
                      ),
                    ],
                  ),

                  // Dec Selection
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text("Declination",
                          style: TextStyle(color: color),
                          textScaler: textScaler(context)),
                    ],
                  ),
                  Wrap(
                    spacing: 4.0,
                    children: [
                      _buildDropdownWithSuffix<String>(
                        value: _decSign,
                        color: color,
                        context: context,
                        items: ['+', '-']
                            .map((e) => DropdownMenuItem(
                                  value: e,
                                  child: Text(e,
                                      style: TextStyle(color: color),
                                      textScaler: textScaler(context)),
                                ))
                            .toList(),
                        onChanged: (val) => setState(() => _decSign = val!),
                        suffix: ' ',
                      ),
                      _buildDropdownWithSuffix<int>(
                        value: _decD,
                        color: color,
                        context: context,
                        items: _buildIntItems(0, 90, color, context),
                        onChanged: (val) {
                          setState(() {
                            _decD = val!;
                            if (_decD == 90) {
                              _decM = 0;
                              _decS = 0;
                            }
                          });
                        },
                        suffix: '° ',
                      ),
                      _buildDropdownWithSuffix<int>(
                        value: _decM,
                        color: _decD == 90 ? color.withAlpha(100) : color,
                        context: context,
                        items: _buildIntItems(
                            0,
                            59,
                            _decD == 90 ? color.withAlpha(100) : color,
                            context),
                        onChanged: _decD == 90
                            ? null
                            : (val) => setState(() => _decM = val!),
                        suffix: '\' ',
                      ),
                      _buildDropdownWithSuffix<int>(
                        value: _decS,
                        color: _decD == 90 ? color.withAlpha(100) : color,
                        context: context,
                        items: _buildIntItems(
                            0,
                            59,
                            _decD == 90 ? color.withAlpha(100) : color,
                            context),
                        onChanged: _decD == 90
                            ? null
                            : (val) => setState(() => _decS = val!),
                        suffix: '" ',
                      ),
                    ],
                  ),

                  const SizedBox(height: 10),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      TextButton.icon(
                        icon: const Icon(Icons.gps_fixed),
                        label: Text("GoTo",
                            style: TextStyle(color: color),
                            textScaler: textScaler(context)),
                        onPressed: _submitGoto,
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
