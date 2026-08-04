import 'package:flutter/material.dart';
import 'package:cedar_flutter/client_main.dart';
import 'draw_catalog.dart';
import 'catalog_browser.dart';

void main() {
  clientMain(
      /*drawCatalogEntries=*/ drawCatalogEntries,
      /*showCatalogBrowser=*/ showCatalogBrowser,
      /*objectInfoDialog=*/ null,
      /*wifiAccessPointDialog=*/ null,
      /*gotoRaDecDialog=*/ null,
      /*updaterInfo=*/ null,
      /*updateServiceAvailable=*/ false);
}
