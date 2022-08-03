#/bin/sh
docker buildx build . -t imgcompress

docker run --rm -iv${PWD}:/host-volume imgcompress sh -s <<EOF
chown $(id -u):$(id -g) /usr/bin/oxipng
cp -a /usr/bin/oxipng /host-volume
chown $(id -u):$(id -g) /usr/bin/jpegtran-static
cp -a /usr/bin/jpegtran-static /host-volume
chown $(id -u):$(id -g) /usr/bin/cjpeg-static
cp -a /usr/bin/cjpeg-static /host-volume
EOF
