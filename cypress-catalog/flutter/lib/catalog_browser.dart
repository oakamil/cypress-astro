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
import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:http/http.dart' as http;
import 'package:cedar_flutter/cedar.pbgrpc.dart' as cedar_rpc;
import 'package:cedar_flutter/cedar.pb.dart' as cedar_pb;
import 'package:cedar_flutter/cedar_sky.pb.dart' as cedar_sky_rpc;
import 'dart:convert';
import 'package:cedar_flutter/client_main.dart';
import 'dart:math' as math;
import 'package:file_selector/file_selector.dart';
import 'package:cedar_flutter/platform.dart';
import 'dart:io';

void showCatalogBrowser(BuildContext context, MyHomePageState state) {
  showDialog(
    context: context,
    builder: (context) => CatalogBrowserDialog(state: state),
  );
}

// File-scoped variables to persist catalog settings across dialog opening/closings during a single app session.
String _sessionSelectedCategory = "Nearest Objects";
String _sessionSortBy = "Distance";
String _sessionVisibilityFilter = "Above Horizon";
String _sessionTypeFilter = "All Types";
int _sessionMagLimit = 20;

class CatalogBrowserDialog extends StatefulWidget {
  final MyHomePageState state;
  const CatalogBrowserDialog({Key? key, required this.state}) : super(key: key);

  @override
  _CatalogBrowserDialogState createState() => _CatalogBrowserDialogState();
}

class _CatalogBrowserDialogState extends State<CatalogBrowserDialog> {
  String _searchQuery = "";

  final TextEditingController _searchController = TextEditingController();
  final FocusNode _searchFocus = FocusNode();
  bool _showKeyboard = false;

  List<cedar_sky_rpc.SelectedCatalogEntry> _results = [];
  bool _isLoading = false;
  bool _isRefreshingComets = false;

  static const List<String> categories = [
    "Nearest Objects",
    "All Objects",
    "Solar System",
    "Bright Stars",
    "Messier",
    "Caldwell",
    "Collinder",
    "NGC",
    "IC",
    "Asteroids",
    "Comets",
    "Herschel 400",
    "Barnard",
    "Sharpless",
    "Abell",
    "Arp",
    "SAC Asterisms",
    "SAC Doubles",
    "SAC Red Stars",
    "TAAS 200",
    "Harris",
    "RASC Doubles",
    "EGC"
  ];

  static const Map<String, String> categoryToShortCode = {
    "Messier": "M",
    "Caldwell": "C",
    "Collinder": "Col",
    "NGC": "NGC",
    "IC": "IC",
    "Asteroids": "Asteroid",
    "Comets": "Comet",
    "Herschel 400": "H",
    "Barnard": "B",
    "Sharpless": "Sh2",
    "Abell": "Abl",
    "Arp": "Arp",
    "SAC Asterisms": "SaA",
    "SAC Doubles": "SaM",
    "SAC Red Stars": "SaR",
    "TAAS 200": "Ta2",
    "Harris": "Har",
    "RASC Doubles": "RDS",
    "EGC": "EGC",
    "Bright Stars": "Str"
  };

  @override
  void initState() {
    super.initState();
    _searchFocus.addListener(() {
      if (_searchFocus.hasFocus) {
        setState(() {
          _showKeyboard = true;
        });
      }
    });
    _fetchResults();
  }

  @override
  void dispose() {
    _searchController.dispose();
    _searchFocus.dispose();
    super.dispose();
  }

  Future<void> _fetchResults() async {
    setState(() {
      _isLoading = true;
    });

    try {
      final client = await getClient();

      final match = cedar_sky_rpc.CatalogEntryMatch()
        ..faintestMagnitude = _sessionMagLimit;

      if (categoryToShortCode.containsKey(_sessionSelectedCategory)) {
        match.catalogLabel.add(categoryToShortCode[_sessionSelectedCategory]!);
        match.matchCatalogLabel = true;
      } else if (_sessionSelectedCategory == "Solar System") {
        match.catalogLabel.add("Planet");
        match.catalogLabel.add("Moon");
        match.catalogLabel.add("Sun");
        match.matchCatalogLabel = true;
      }

      final request = cedar_sky_rpc.QueryCatalogRequest()
        ..limitResult = 300
        ..catalogEntryMatch = match;

      if (_searchQuery.isNotEmpty) {
        request.textSearch = _searchQuery;
      }

      if (_sessionVisibilityFilter == "Above Horizon") {
        request.minElevation = 0.0;
      }

      if (_sessionTypeFilter != "All Types") {
        match.objectTypeLabel.add(_sessionTypeFilter);
        match.matchObjectTypeLabel = true;
      }

      var orderProto = cedar_sky_rpc.Ordering.UNSPECIFIED;
      // HACK: We smuggle Name and Catalog ID sorts through `decrowdDistance`
      // because cedar-server drops unrecognized Ordering enum values.
      // decrowdDistance = -1.0 -> Name, -2.0 -> Catalog ID
      if (_sessionSortBy == "Name") {
        request.decrowdDistance = -1.0;
      } else if (_sessionSortBy == "Catalog ID") {
        request.decrowdDistance = -2.0;
      } else if (_sessionSortBy == "Brightness") {
        orderProto = cedar_sky_rpc.Ordering.BRIGHTNESS;
      } else if (_sessionSortBy == "Distance" ||
          _sessionSelectedCategory == "Nearest Objects") {
        orderProto = cedar_sky_rpc.Ordering.SKY_LOCATION;
      }

      request.ordering = orderProto;

      final response = await client.queryCatalogEntries(request);

      if (mounted) {
        setState(() {
          _results = response.entries.toList();
        });
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Failed to load catalog: $e')),
        );
      }
    } finally {
      if (mounted) {
        setState(() {
          _isLoading = false;
        });
      }
    }
  }

  void _showSettingsDialog() {
    showDialog(
      context: context,
      builder: (BuildContext context) {
        final color = Theme.of(context).colorScheme.primary;
        return StatefulBuilder(builder: (context, setDialogState) {
          return AlertDialog(
            backgroundColor: Colors.black,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(10),
              side: BorderSide(color: color),
            ),
            title: Text("Catalog Settings", style: TextStyle(color: color)),
            content: SingleChildScrollView(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text("Sort By",
                      style:
                          TextStyle(color: color, fontWeight: FontWeight.bold)),
                  DropdownButton<String>(
                    value: _sessionSortBy,
                    dropdownColor: Colors.black,
                    style: TextStyle(color: color),
                    isExpanded: true,
                    underline: Container(height: 1, color: color),
                    onChanged: (String? newValue) {
                      if (newValue != null) {
                        setDialogState(() {
                          _sessionSortBy = newValue;
                        });
                        setState(() {
                          _sessionSortBy = newValue;
                        });
                      }
                    },
                    items: <String>[
                      "Catalog ID",
                      "Name",
                      "Brightness",
                      "Distance"
                    ]
                        .map((String value) => DropdownMenuItem<String>(
                            value: value, child: Text(value)))
                        .toList(),
                  ),
                  const SizedBox(height: 16),
                  Text("Visibility",
                      style:
                          TextStyle(color: color, fontWeight: FontWeight.bold)),
                  DropdownButton<String>(
                    value: _sessionVisibilityFilter,
                    dropdownColor: Colors.black,
                    style: TextStyle(color: color),
                    isExpanded: true,
                    underline: Container(height: 1, color: color),
                    onChanged: (String? newValue) {
                      if (newValue != null) {
                        setDialogState(() {
                          _sessionVisibilityFilter = newValue;
                        });
                        setState(() {
                          _sessionVisibilityFilter = newValue;
                        });
                      }
                    },
                    items: <String>["All Objects", "Above Horizon"]
                        .map((String value) => DropdownMenuItem<String>(
                            value: value, child: Text(value)))
                        .toList(),
                  ),
                  const SizedBox(height: 16),
                  Text("Object Type",
                      style:
                          TextStyle(color: color, fontWeight: FontWeight.bold)),
                  DropdownButton<String>(
                    value: _sessionTypeFilter,
                    dropdownColor: Colors.black,
                    style: TextStyle(color: color),
                    isExpanded: true,
                    underline: Container(height: 1, color: color),
                    onChanged: (String? newValue) {
                      if (newValue != null) {
                        setDialogState(() {
                          _sessionTypeFilter = newValue;
                        });
                        setState(() {
                          _sessionTypeFilter = newValue;
                        });
                      }
                    },
                    items: <String>[
                      "All Types",
                      "Star",
                      "Galaxy",
                      "Nebula",
                      "Cluster",
                      "Planet",
                      "Asteroid",
                      "Comet"
                    ]
                        .map((String value) => DropdownMenuItem<String>(
                            value: value, child: Text(value)))
                        .toList(),
                  ),
                  const SizedBox(height: 16),
                  Text("Magnitude Limit: $_sessionMagLimit",
                      style:
                          TextStyle(color: color, fontWeight: FontWeight.bold)),
                  Slider(
                    value: _sessionMagLimit.toDouble(),
                    min: 0,
                    max: 30,
                    divisions: 30,
                    activeColor: color,
                    inactiveColor: color.withAlpha(100),
                    onChanged: (double newValue) {
                      setDialogState(() {
                        _sessionMagLimit = newValue.toInt();
                      });
                      setState(() {
                        _sessionMagLimit = newValue.toInt();
                      });
                    },
                  ),
                ],
              ),
            ),
            actions: [
              TextButton(
                child: Text("Close", style: TextStyle(color: color)),
                onPressed: () {
                  Navigator.of(context).pop();
                  _fetchResults(); // Fetch again with new settings
                },
              ),
            ],
          );
        });
      },
    );
  }

  Future<void> _refreshComets() async {
    try {
      final XFile? result = await openFile();

      if (result != null) {
        setState(() {
          _isRefreshingComets = true;
        });

        final bytes = await result.readAsBytes();
        final data = utf8.decode(bytes, allowMalformed: true);

        final host = kIsWeb ? Uri.base.host : await resolveCedarHost();
        final postUri = Uri.parse("http://$host:8081/update-comets");
        final postResponse = await http.post(postUri, body: data);

        if (postResponse.statusCode == 200) {
          // Success, refetch results
          await widget.state.setServerTime(DateTime.now());
          _fetchResults();
        } else {
          debugPrint("Failed to post comet data: ${postResponse.statusCode}");
        }
      }
    } catch (e) {
      debugPrint("Error refreshing comets: $e");
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text("Error: $e")));
      }
    } finally {
      if (mounted) {
        setState(() {
          _isRefreshingComets = false;
        });
      }
    }
  }

  List<double>? _raDecToAltAz(double raDeg, double decDeg) {
    if (!widget.state.fixedSettings.hasObserverLocation()) return null;
    double lat = widget.state.fixedSettings.observerLocation.latitude;
    double lon = widget.state.fixedSettings.observerLocation.longitude;
    double timeMs = DateTime.now().millisecondsSinceEpoch.toDouble();
    double daysSinceJ2000 = (timeMs - 946728000000.0) / 86400000.0;
    double gmst = (280.46061837 + 360.98564736629 * daysSinceJ2000) % 360.0;
    if (gmst < 0) gmst += 360.0;
    double lst = (gmst + lon) % 360.0;
    if (lst < 0) lst += 360.0;
    double lstRad = lst * math.pi / 180.0;
    double latRad = lat * math.pi / 180.0;
    double sinLat = math.sin(latRad);
    double cosLat = math.cos(latRad);

    double decRad = decDeg * math.pi / 180.0;
    double raRad = raDeg * math.pi / 180.0;

    double altSin = math.sin(decRad) * sinLat +
        math.cos(decRad) * cosLat * math.cos(lstRad - raRad);
    double altRad = math.asin(altSin.clamp(-1.0, 1.0));
    double cosAlt = math.cos(altRad);

    double azRad = 0.0;
    if (cosAlt.abs() > 1e-6) {
      double azCos = (math.sin(decRad) - altSin * sinLat) / (cosAlt * cosLat);
      double azSin = -(math.sin(lstRad - raRad) * math.cos(decRad)) / cosAlt;
      azRad = math.atan2(azSin, azCos);
    }
    return [altRad, azRad];
  }

  bool isBelowHorizon(double ra, double dec) {
    if (!widget.state.fixedSettings.hasObserverLocation()) return false;
    double lat = widget.state.fixedSettings.observerLocation.latitude;
    double lon = widget.state.fixedSettings.observerLocation.longitude;
    double timeMs = DateTime.now().millisecondsSinceEpoch.toDouble();
    double daysSinceJ2000 = (timeMs - 946728000000.0) / 86400000.0;
    double gmst = (280.46061837 + 360.98564736629 * daysSinceJ2000) % 360.0;
    if (gmst < 0) gmst += 360.0;
    double lst = (gmst + lon) % 360.0;
    if (lst < 0) lst += 360.0;
    double lstRad = lst * math.pi / 180.0;
    double latRad = lat * math.pi / 180.0;
    double sinLat = math.sin(latRad);
    double cosLat = math.cos(latRad);

    double decRad = dec * math.pi / 180.0;
    double raRad = ra * math.pi / 180.0;
    double altSin = math.sin(decRad) * sinLat +
        math.cos(decRad) * cosLat * math.cos(lstRad - raRad);
    return altSin <= 0;
  }

  @override
  Widget build(BuildContext context) {
    final color = Theme.of(context).colorScheme.primary;
    final size = MediaQuery.of(context).size;

    return Dialog(
      backgroundColor: Colors.transparent,
      insetPadding: const EdgeInsets.all(20),
      child: Container(
        width: size.width * 0.9,
        height: size.height * 0.9,
        clipBehavior: Clip.antiAlias,
        decoration: BoxDecoration(
          color: Colors.black,
          borderRadius: BorderRadius.circular(10),
          border: Border.all(color: color),
        ),
        child: Scaffold(
          backgroundColor: Colors.transparent,
          drawer: Drawer(
            backgroundColor: Colors.black,
            shape:
                const RoundedRectangleBorder(borderRadius: BorderRadius.zero),
            child: ListView.builder(
              itemCount: categories.length + 1,
              itemBuilder: (context, index) {
                if (index == 0) {
                  return DrawerHeader(
                    decoration: BoxDecoration(
                        border: Border(bottom: BorderSide(color: color))),
                    child: Align(
                      alignment: Alignment.bottomLeft,
                      child: Text("Catalogs",
                          style: TextStyle(
                              color: color,
                              fontSize: 24,
                              fontWeight: FontWeight.bold)),
                    ),
                  );
                }
                final val = categories[index - 1];
                return ListTile(
                  title: Text(val, style: TextStyle(color: color)),
                  selected: _sessionSelectedCategory == val,
                  selectedTileColor: color.withAlpha(50),
                  onTap: () {
                    setState(() {
                      _sessionSelectedCategory = val;
                      _fetchResults();
                    });
                    Navigator.pop(context);
                  },
                );
              },
            ),
          ),
          body: Builder(builder: (BuildContext innerContext) {
            return Padding(
              padding: const EdgeInsets.all(16.0),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Row(
                    children: [
                      IconButton(
                        icon: Icon(Icons.menu, color: color),
                        onPressed: () {
                          Scaffold.of(innerContext).openDrawer();
                        },
                      ),
                      const SizedBox(width: 10),
                      Expanded(
                        child: TextField(
                          controller: _searchController,
                          focusNode: _searchFocus,
                          readOnly: false,
                          keyboardType: TextInputType.none,
                          showCursor: true,
                          style: TextStyle(color: color),
                          decoration: InputDecoration(
                            hintText: "Search $_sessionSelectedCategory...",
                            hintStyle: TextStyle(color: color.withAlpha(150)),
                            enabledBorder: OutlineInputBorder(
                                borderSide: BorderSide(color: color)),
                            focusedBorder: OutlineInputBorder(
                                borderSide: BorderSide(color: color, width: 2)),
                            prefixIcon: Icon(Icons.search, color: color),
                          ),
                          onTap: () {
                            setState(() {
                              _showKeyboard = true;
                            });
                          },
                          onChanged: (val) {
                            _searchQuery = val;
                            _fetchResults();
                          },
                        ),
                      ),
                      if (_sessionSelectedCategory == "Comets") ...[
                        const SizedBox(width: 8),
                        IconButton(
                          icon: _isRefreshingComets
                              ? SizedBox(
                                  width: 20,
                                  height: 20,
                                  child: CircularProgressIndicator(
                                      strokeWidth: 2, color: color))
                              : Icon(Icons.file_upload, color: color),
                          onPressed:
                              _isRefreshingComets ? null : _refreshComets,
                          tooltip: "Upload MPC Comets File",
                        ),
                      ],
                      const SizedBox(width: 10),
                      IconButton(
                        icon: Icon(Icons.settings, color: color),
                        onPressed: _showSettingsDialog,
                      ),
                      IconButton(
                        icon: Icon(Icons.close, color: color),
                        onPressed: () => Navigator.of(context).pop(),
                      )
                    ],
                  ),
                  const SizedBox(height: 16),
                  Expanded(
                    child: _isLoading
                        ? Center(child: CircularProgressIndicator(color: color))
                        : ListView.builder(
                            itemCount: _results.length,
                            itemBuilder: (context, index) {
                              final sel = _results[index];
                              final entry = sel.entry;
                              bool belowHorizon = false;
                              if (entry.hasCoord() &&
                                  isBelowHorizon(
                                      entry.coord.ra, entry.coord.dec)) {
                                belowHorizon = true;
                              }

                              String topLabel = "";
                              String bottomLabel = "";

                              String catString = "";
                              if (entry.catalogLabel == "Str" ||
                                  entry.catalogLabel == "Planet" ||
                                  entry.catalogLabel == "Solar System" ||
                                  entry.catalogLabel == "Asteroid" ||
                                  entry.catalogLabel == "Comet") {
                                catString = entry.catalogEntry;
                              } else {
                                catString =
                                    "${entry.catalogLabel} ${entry.catalogEntry}";
                              }
                              catString = catString.trim();

                              if (entry.hasCommonName() &&
                                  entry.commonName.isNotEmpty) {
                                topLabel = entry.commonName;
                                bottomLabel =
                                    "$catString • ${entry.objectType.label}";
                              } else {
                                topLabel = catString;
                                bottomLabel = entry.objectType.label;
                              }

                              String constelStr = "";
                              if (entry.hasConstellation() &&
                                  entry.constellation.label.isNotEmpty) {
                                constelStr = " in ${entry.constellation.label}";
                              }
                              bottomLabel += constelStr;

                              String magStr = "";
                              if (entry.hasMagnitude() &&
                                  entry.magnitude < 90.0) {
                                magStr =
                                    " • Mag: ${entry.magnitude.toStringAsFixed(1)}";
                              } else if (entry.hasMagnitude()) {
                                magStr = " • Mag: N/A";
                              }
                              bottomLabel += magStr;

                              Color cardColor = belowHorizon
                                  ? const Color(0xFF4A1010).withAlpha(150)
                                  : Colors.transparent;
                              Color borderColor = belowHorizon
                                  ? const Color(0xFF4A1010)
                                  : color.withAlpha(100);

                              return Card(
                                color: cardColor,
                                shape: RoundedRectangleBorder(
                                    side: BorderSide(color: borderColor),
                                    borderRadius: BorderRadius.circular(8)),
                                child: ListTile(
                                  title: Text(topLabel,
                                      style: TextStyle(
                                          color: color,
                                          fontWeight: FontWeight.bold)),
                                  subtitle: Text(bottomLabel,
                                      style: TextStyle(
                                          color: color.withAlpha(180))),
                                  trailing: Row(
                                    mainAxisSize: MainAxisSize.min,
                                    children: [
                                      if (widget.state.hasSolution &&
                                          entry.hasCoord())
                                        ...(() {
                                          double ra1 = widget.state.solutionRA;
                                          double dec1 =
                                              widget.state.solutionDec;
                                          double ra2 = entry.coord.ra;
                                          double dec2 = entry.coord.dec;

                                          double r1 = ra1 * math.pi / 180.0;
                                          double d1 = dec1 * math.pi / 180.0;
                                          double r2 = ra2 * math.pi / 180.0;
                                          double d2 = dec2 * math.pi / 180.0;
                                          double dlon = r2 - r1;
                                          double dlat = d2 - d1;

                                          double a = math.pow(
                                                  math.sin(dlat / 2), 2) +
                                              math.cos(d1) *
                                                  math.cos(d2) *
                                                  math.pow(
                                                      math.sin(dlon / 2), 2);
                                          double c =
                                              2 * math.asin(math.sqrt(a));
                                          double dist = c * 180.0 / math.pi;

                                          double finalAngle = 0.0;
                                          bool isAltAz = widget.state
                                                  .preferences?.mountType ==
                                              cedar_rpc.MountType.ALT_AZ;

                                          if (isAltAz &&
                                              widget.state.fixedSettings
                                                  .hasObserverLocation()) {
                                            final altAz1 =
                                                _raDecToAltAz(ra1, dec1);
                                            final altAz2 =
                                                _raDecToAltAz(ra2, dec2);
                                            if (altAz1 != null &&
                                                altAz2 != null) {
                                              double dAz =
                                                  altAz2[1] - altAz1[1];
                                              double dAlt =
                                                  altAz2[0] - altAz1[0];
                                              double sinTermAz =
                                                  math.sin(dAz / 2);
                                              double yy = math.sin(dAlt) +
                                                  2.0 *
                                                      math.sin(altAz1[0]) *
                                                      math.cos(altAz2[0]) *
                                                      sinTermAz *
                                                      sinTermAz;
                                              double xx = math.cos(altAz2[0]) *
                                                  math.sin(dAz);
                                              double bearing =
                                                  math.atan2(xx, yy);
                                              finalAngle = bearing;
                                            }
                                          } else {
                                            double sinTerm = math.sin(dlon / 2);
                                            double yy = math.sin(dlat) +
                                                2.0 *
                                                    math.sin(d1) *
                                                    math.cos(d2) *
                                                    sinTerm *
                                                    sinTerm;
                                            double xx =
                                                math.cos(d2) * math.sin(dlon);
                                            double bearing = math.atan2(xx, yy);
                                            double roll =
                                                widget.state.solutionRollAngle *
                                                    math.pi /
                                                    180.0;
                                            finalAngle = -(bearing + roll);
                                          }

                                          return [
                                            Transform.rotate(
                                              angle: finalAngle,
                                              child: Icon(Icons.arrow_upward,
                                                  color: color, size: 20),
                                            ),
                                            const SizedBox(width: 4),
                                            Text("${dist.toStringAsFixed(1)}°",
                                                style: TextStyle(
                                                    color: color,
                                                    fontSize: 14)),
                                            const SizedBox(width: 12),
                                          ];
                                        })(),
                                      if (belowHorizon)
                                        Padding(
                                          padding:
                                              const EdgeInsets.only(right: 8.0),
                                          child: Text("Below Horizon",
                                              style: TextStyle(
                                                  color: color.withAlpha(150),
                                                  fontSize: 12,
                                                  fontWeight: FontWeight.bold)),
                                        )
                                      else
                                        InkWell(
                                          borderRadius:
                                              BorderRadius.circular(8),
                                          onTap: () {
                                            if (entry.hasCoord()) {
                                              final req =
                                                  cedar_pb.ActionRequest()
                                                    ..initiateSlew =
                                                        entry.coord;
                                              widget.state.initiateAction(req);
                                              Navigator.of(context).pop();
                                            }
                                          },
                                          child: Padding(
                                            padding: const EdgeInsets.all(8.0),
                                            child: Column(
                                              mainAxisSize: MainAxisSize.min,
                                              children: [
                                                Icon(Icons.gps_fixed,
                                                    color: color, size: 24),
                                                Text("GoTo",
                                                    style: TextStyle(
                                                        color: color,
                                                        fontSize: 10)),
                                              ],
                                            ),
                                          ),
                                        ),
                                    ],
                                  ),
                                ),
                              );
                            },
                          ),
                  ),
                  if (_showKeyboard)
                    CustomKeyboard(
                      onKeyClick: (char) {
                        setState(() {
                          _searchController.text += char;
                          _searchQuery = _searchController.text;
                          _fetchResults();
                        });
                      },
                      onSpace: () {
                        setState(() {
                          _searchController.text += " ";
                          _searchQuery = _searchController.text;
                          _fetchResults();
                        });
                      },
                      onBackspace: () {
                        if (_searchController.text.isNotEmpty) {
                          setState(() {
                            _searchController.text = _searchController.text
                                .substring(
                                    0, _searchController.text.length - 1);
                            _searchQuery = _searchController.text;
                            _fetchResults();
                          });
                        }
                      },
                    ),
                ],
              ),
            );
          }),
        ),
      ),
    );
  }
}

class CustomKeyboard extends StatelessWidget {
  final Function(String) onKeyClick;
  final VoidCallback onSpace;
  final VoidCallback onBackspace;

  const CustomKeyboard({
    Key? key,
    required this.onKeyClick,
    required this.onSpace,
    required this.onBackspace,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    final color = Theme.of(context).colorScheme.primary;
    final keys = [
      ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"],
      ["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"],
      ["a", "s", "d", "f", "g", "h", "j", "k", "l"],
      ["␣", "z", "x", "c", "v", "b", "n", "m", "⌫"]
    ];

    return Container(
      color: Colors.black,
      padding: const EdgeInsets.all(2.0),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: keys.map((row) {
          return Padding(
            padding: const EdgeInsets.symmetric(vertical: 2.0),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                if (row.length < 10) Spacer(flex: 10 - row.length),
                ...row.map((key) {
                  return Expanded(
                    flex: 2,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 2.0),
                      child: Material(
                        color: Colors.transparent,
                        child: InkWell(
                          onTap: () {
                            if (key == "␣") {
                              onSpace();
                            } else if (key == "⌫") {
                              onBackspace();
                            } else {
                              onKeyClick(key);
                            }
                          },
                          child: Container(
                            height: 36,
                            decoration: BoxDecoration(
                              border: Border.all(color: color),
                              borderRadius: BorderRadius.circular(4),
                            ),
                            alignment: Alignment.center,
                            child: Text(
                              key,
                              style: TextStyle(color: color, fontSize: 16),
                            ),
                          ),
                        ),
                      ),
                    ),
                  );
                }),
                if (row.length < 10) Spacer(flex: 10 - row.length),
              ],
            ),
          );
        }).toList(),
      ),
    );
  }
}
