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
import 'package:cedar_flutter/cedar_sky.pb.dart' as cedar_sky_rpc;
import 'package:cedar_flutter/cedar.pb.dart' as cedar_pb;
import 'package:cedar_flutter/settings.dart';

/// Wired up in client_main.dart
void showObjectInfoDialog(MyHomePageState state, BuildContext context,
    cedar_sky_rpc.SelectedCatalogEntry selEntry,
    {bool dedupedEntriesMayBeIncomplete = false,
    Iterable<String>? preferredCatalogs,
    String? searchText}) {
  showDialog(
    context: context,
    builder: (context) => ObjectInfoDialog(state: state, selEntry: selEntry),
  );
}

class ObjectInfoDialog extends StatelessWidget {
  final MyHomePageState state;
  final cedar_sky_rpc.SelectedCatalogEntry selEntry;

  const ObjectInfoDialog(
      {Key? key, required this.state, required this.selEntry})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    final color = Theme.of(context).colorScheme.primary;
    final entry = selEntry.entry;

    // --- Label Logic ---
    String catString = "";
    if (entry.catalogLabel == "Str" ||
        entry.catalogLabel == "Planet" ||
        entry.catalogLabel == "Solar System" ||
        entry.catalogLabel == "Asteroid" ||
        entry.catalogLabel == "Comet") {
      catString = entry.catalogEntry;
    } else {
      catString = "${entry.catalogLabel} ${entry.catalogEntry}";
    }
    catString = catString.trim();

    String title = "";
    String typeStr = entry.objectType.label;

    // Determine main title
    if (entry.hasCommonName() && entry.commonName.isNotEmpty) {
      title = entry.commonName;
    } else {
      title = catString;
    }

    // Determine subtitle (Type in Constellation, or just Type if missing)
    String constelStr = "";
    if (entry.hasConstellation() && entry.constellation.label.isNotEmpty) {
      constelStr = " in ${entry.constellation.label}";
    }
    String subtitle = "$typeStr$constelStr";

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
                  Text(title,
                      style:
                          TextStyle(color: color, fontWeight: FontWeight.bold),
                      textScaler: textScaler(context)),
                ]),
                if (subtitle.isNotEmpty)
                  Row(mainAxisAlignment: MainAxisAlignment.center, children: [
                    Text(subtitle,
                        style: TextStyle(color: color.withAlpha(180)),
                        textScaler: textScaler(context)),
                  ]),
                const SizedBox(height: 5),

                if (entry.hasCommonName() && entry.commonName.isNotEmpty)
                  _buildDetailRow("Catalog", catString, color, context),

                if (entry.hasMagnitude() && entry.magnitude < 90.0)
                  _buildDetailRow("Magnitude",
                      entry.magnitude.toStringAsFixed(2), color, context),

                if (entry.angularSize.isNotEmpty)
                  _buildDetailRow(
                      "Angular Size", "${entry.angularSize}′", color, context),

                if (entry.hasCoord()) ...[
                  _buildDetailRow(
                      "RA",
                      state.formatRightAscension(entry.coord.ra),
                      color,
                      context),
                  _buildDetailRow("Dec",
                      state.formatDeclination(entry.coord.dec), color, context),
                ],

                if (selEntry.hasAltitude())
                  _buildDetailRow("Altitude",
                      state.formatAltitude(selEntry.altitude), color, context),

                if (selEntry.hasAzimuth())
                  _buildDetailRow("Azimuth",
                      state.formatAzimuth(selEntry.azimuth), color, context),

                const SizedBox(height: 10),

                // Actions
                if (entry.hasCoord())
                  Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      TextButton.icon(
                        icon: const Icon(Icons.gps_fixed),
                        label: Text("GoTo",
                            style: TextStyle(color: color),
                            textScaler: textScaler(context)),
                        onPressed: () {
                          final req = cedar_pb.ActionRequest()
                            ..initiateSlew = entry.coord;
                          state.initiateAction(req);
                          Navigator.of(context).pop();
                        },
                      ),
                    ],
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _buildDetailRow(
      String label, String value, Color color, BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Text(label,
            style: TextStyle(color: color), textScaler: textScaler(context)),
        Text(value,
            style: TextStyle(color: color), textScaler: textScaler(context)),
      ],
    );
  }
}
