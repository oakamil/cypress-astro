#/bin/bash

cp -rf cypress /home/cedar/
sudo cp cypress-display.service /lib/systemd/system/
sudo systemctl enable cypress-display
sudo systemctl start cypress-display
