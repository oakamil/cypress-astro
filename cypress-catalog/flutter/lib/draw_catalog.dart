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
import 'package:cedar_flutter/cedar.pb.dart' as cedar_rpc;
import 'package:cedar_flutter/draw_util.dart';

void drawCatalogEntries(
    BuildContext context,
    Canvas canvas,
    Color color,
    List<cedar_rpc.FovCatalogEntry> entries,
    bool drawLabels,
    int binFactor,
    cedar_rpc.FovCatalogEntry? target,
    [Iterable<String>? preferredCatalogs]) {
  final paintColor = color;

  for (final fovEntry in entries) {
    if (fovEntry.hasImagePos()) {
      final pos = Offset(fovEntry.imagePos.x.toDouble() / binFactor,
          fovEntry.imagePos.y.toDouble() / binFactor);

      // Target object gets a cross, others get a gap cross or circle
      if (target != null &&
          target.entry.catalogLabel == fovEntry.entry.catalogLabel &&
          target.entry.catalogEntry == fovEntry.entry.catalogEntry) {
        drawCross(canvas, paintColor, pos, 15.0, 0.0, 2.0, 2.0);
      } else {
        // Just draw a small circle or cross
        drawGapCross(canvas, paintColor, pos, 10.0, 3.0, 0.0, 1.5, 1.5);
      }

      if (drawLabels) {
        final label = fovEntry.entry.hasCommonName() &&
                fovEntry.entry.commonName.isNotEmpty
            ? fovEntry.entry.commonName
            : "${fovEntry.entry.catalogLabel}${fovEntry.entry.catalogEntry}";
        drawText(
            context, canvas, paintColor, Offset(pos.dx, pos.dy - 20.0), label);
      }
    }
  }
}
