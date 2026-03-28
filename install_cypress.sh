#!/bin/bash

set -e  # Exit on any error

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <path to cedar input image>"
    exit 1
fi

CEDAR_IMAGE_FILE="$1"

MOUNT_POINT="$HOME/mnt/rpi_root"
BOOT_MOUNT_POINT="$HOME/mnt/rpi_boot"
BIN_DIR=$MOUNT_POINT/home/cedar/cedar/bin
CEDAR_AIM_DIR=$MOUNT_POINT/home/cedar/cedar/cedar-aim/cedar_flutter/build

echo
echo "Mounting image $CEDAR_IMAGE_FILE"
mkdir -p "$MOUNT_POINT"
LOOP_DEV=$(sudo losetup -fP --show "$CEDAR_IMAGE_FILE")

if [ -z "$LOOP_DEV" ]; then
    echo "Failed to create loop device."
    exit 1
fi

echo
echo "Using device: ${LOOP_DEV}p2"
sudo mount "${LOOP_DEV}p2" "$MOUNT_POINT"

echo
echo "Copying Cypress server binary"
sudo cp dist/cypress-server $BIN_DIR/.

echo
echo "Disabling Cedar service"
sudo systemctl --root=$ROOT disable cedar.service

echo
echo "Updating Cedar service to run Cypress server"
sudo bash -c "cat > $ROOT/lib/systemd/system/cedar.service <<EOF
[Unit]
Description=Cedar Server
After=NetworkManager.service network-online.target cedar-ap-setup.service
Wants=NetworkManager.service network-online.target
Wants=cedar-ap-setup.service

[Service]
User=cedar
WorkingDirectory=/home/cedar/run
Type=simple
ExecStart=/bin/bash -c '/home/cedar/cedar/bin/cypress-server'

[Install]
WantedBy=multi-user.target
EOF"

echo
echo "Enable Cedar service"
sudo systemctl --root=$MOUNT_POINT enable cedar.service

echo
echo "Set caps on Cypress server binary"
caps="cap_sys_time,cap_dac_override,cap_chown,cap_fowner,cap_net_bind_service+ep"
sudo setcap "$caps" $BIN_DIR/cypress-server

echo
echo "Removing existing Cedar-Aim"
sudo rm -rf $CEDAR_AIM_DIR/*

echo
echo "Copying Cedar-Aim"
sudo cp -R ../cedar-aim/cedar_flutter/build/web $CEDAR_AIM_DIR/.

echo
echo "Using device: ${LOOP_DEV}p1"
mkdir -p "$BOOT_MOUNT_POINT"
sudo mount "${LOOP_DEV}p1" "$BOOT_MOUNT_POINT"

echo
echo "Adding camera configuration"
sudo bash -c 'printf "\ndtoverlay=imx290,clock-frequency=74250000\n" >> $BOOT_MOUNT_POINT/config.txt'

echo
echo "Unmounting image"
sync
sudo umount "$MOUNT_POINT"
sudo umount "$BOOT_MOUNT_POINT"
sleep 3
sudo losetup -d "$LOOP_DEV"

echo
echo "Image update complete"
