import 'package:flutter/material.dart';
import 'package:cedar_flutter/client_main.dart';
import 'catalog_browser.dart';
import 'draw_catalog.dart';
import 'goto_target.dart';
import 'object_info.dart';

void main() {
  clientMain(
      /*drawCatalogEntries=*/ drawCatalogEntries,
      /*showCatalogBrowser=*/ showCatalogBrowser,
      /*objectInfoDialog=*/ showObjectInfoDialog,
      /*wifiAccessPointDialog=*/ null,
      /*gotoRaDecDialog=*/ gotoRaDecDialog,
      /*updaterInfo=*/ null,
      /*updateServiceAvailable=*/ false);
}
