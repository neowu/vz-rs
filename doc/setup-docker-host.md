# DO NOT just copy and run, please check what the script doing and adapt to your own setup

alpine doesn't support rosetta, use debian instead if you need rosetta support

# create alpine vm
1. download alpine virt iso `https://dl-cdn.alpinelinux.org/alpine/v3.18/releases/aarch64/alpine-virt-3.18.4-aarch64.iso`
2. vz create alpine --disk-size=500
3. edit ~/.vm/alpine/config.json to add home share
```
{
  "os": "linux",
  "cpu": 12,
  "memory": 4294967296,
  "macAddress": "f6:db:b3:ec:f9:3f",
  "sharing": {
    "[yourname]": "~"
  }
}
```
4. vz run alpine --gui --mount

# setup thru gui / root
```sh
setup-alpine -q
setup-disk
setup-timezone
setup-sshd
setup-user

adduser [yourname] wheel
apk add doas-sudo-shim
echo 'permit nopass :wheel' > /etc/doas.d/doas.conf
```

# setup locally
1. edit `/etc/hosts` to add alpine record
```sh
ssh-copy-id alpine

docker context create vz --docker host=ssh://[yourname]@alpine
```
edit `~/.ssh/config` to add following (for multiplex ssh connection, e.g. VSCode devcontainer runs multiple docker commands simultaneously)
```
Host *
  IdentityFile ~/.ssh/id_ed25519
  ForwardAgent yes
  ControlMaster auto
  ControlPath ~/.cache/ssh/%r@%h-%p
  ControlPersist 600
```

# setup alpine
```sh
sudo sed -i -e 's/GRUB_TIMEOUT=.*/GRUB_TIMEOUT=0/' /etc/default/grub
sudo grub-mkconfig -o /boot/grub/grub.cfg

sudo mkdir /Users
# mount -t virtiofs com.apple.virtio-fs.automount /Users
sudo sed -i -e '$acom.apple.virtio-fs.automount /Users virtiofs rw 0 2' /etc/fstab

sudo sed -i -e 's/.MaxSessions \d*/MaxSessions 256/' -e 's/.MaxStartups .*/MaxStartups 128:30:256/' /etc/ssh/sshd_config
sudo service sshd restart

# power off
echo 'gpio-pl061' | sudo tee /etc/modules-load.d/gpio-pl061.conf
sudo modprobe gpio-pl061

# install docker
sudo sed -i -e '/community/ s/#//' /etc/apk/repositories
sudo apk add docker
sudo addgroup [yourname] docker
sudo rc-update add docker default
sudo service docker start
```

# resize disk if needed in future
1. vz resize alpine --disk-size [newSize]
2. expand disk in alpine
```
apk add cfdisk e2fsprogs-extra
cfdisk
resize2fs /dev/vda3
```
