import 'package:flutter/material.dart';
import 'package:cedar_flutter/client_main.dart';
import 'catalog_browser.dart';
import 'draw_catalog.dart';
import 'goto_target.dart';
import 'object_info.dart';
import 'updater_info.dart';
import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:cedar_flutter/platform.dart';
import 'package:http/http.dart' as http;

void main() {
  clientMain(
      /*drawCatalogEntries=*/ drawCatalogEntries,
      /*showCatalogBrowser=*/ showCatalogBrowser,
      /*objectInfoDialog=*/ showObjectInfoDialog,
      /*wifiAccessPointDialog=*/ null,
      /*gotoRaDecDialog=*/ gotoRaDecDialog,
      /*updaterInfo=*/ UpdaterInfo(
        updateServerSoftwareDialogFunction: showUpdaterInfoDialog,
        restartCedarServerFunction: () async {
          final host = kIsWeb ? Uri.base.host : await resolveCedarHost();
          final postUri = Uri.parse("http://$host:8081/restart-system");
          try {
            await http.post(postUri);
          } catch (e) {
            debugPrint("Error triggering restart: $e");
          }
        },
      ),
      /*updateServiceAvailable=*/ true);
}
