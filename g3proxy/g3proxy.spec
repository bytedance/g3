%if 0%{?rhel} > 7
%undefine _debugsource_packages
%define pkgconfig_real pkgconf
%define cmake_real cmake
%endif

%if 0%{?rhel} == 7
%global debug_package %{nil}
%define pkgconfig_real pkgconfig
%define cmake_real cmake3
%endif

%define build_profile release-lto

Name:           g3proxy
Version:        1.7.33
Release:        1%{?dist}
Summary:        Generic proxy for G3 Project

License:        Apache-2.0
URL:            https://github.com/bytedance/g3
Source0:        %{name}-%{version}.tar.xz

BuildRequires:  gcc, make, %{pkgconfig_real}, %{cmake_real}, capnproto
BuildRequires:  lua-devel, openssl-devel
BuildRequires:  perl-IPC-Cmd
Requires:       systemd
Requires:       ca-certificates

%description
Generic proxy for G3 Project


%prep
%autosetup


%build
G3_PACKAGE_VERSION="%{version}-%{release}"
export G3_PACKAGE_VERSION
LUA_VERSION=$(pkg-config --variable=V lua | tr -d '.')
LUA_FEATURE=lua$LUA_VERSION
SSL_FEATURE=$(sh scripts/package/detect_openssl_feature.sh)
CARES_FEATURE=$(sh scripts/package/detect_c-ares_feature.sh)
export CMAKE="%{cmake_real}"
cargo build --frozen --offline --profile %{build_profile} --no-default-features --features $LUA_FEATURE,$SSL_FEATURE,quic,$CARES_FEATURE,hickory,geoip --package g3proxy --package g3proxy-ctl --package g3proxy-ftp --package g3proxy-lua --package g3proxy-geoip
sh %{name}/service/generate_systemd.sh


%install
rm -rf $RPM_BUILD_ROOT
install -m 755 -D target/%{build_profile}/g3proxy %{buildroot}%{_bindir}/g3proxy
install -m 755 -D target/%{build_profile}/g3proxy-ctl %{buildroot}%{_bindir}/g3proxy-ctl
install -m 755 -D target/%{build_profile}/g3proxy-ftp %{buildroot}%{_bindir}/g3proxy-ftp
install -m 755 -D target/%{build_profile}/g3proxy-lua %{buildroot}%{_bindir}/g3proxy-lua
install -m 755 -D target/%{build_profile}/g3proxy-geoip %{buildroot}%{_bindir}/g3proxy-geoip
install -m 644 -D %{name}/service/g3proxy@.service %{buildroot}/lib/systemd/system/g3proxy@.service


%files
%{_bindir}/g3proxy
%{_bindir}/g3proxy-ctl
%{_bindir}/g3proxy-ftp
%{_bindir}/g3proxy-lua
%{_bindir}/g3proxy-geoip
/lib/systemd/system/g3proxy@.service
%license LICENSE
%license LICENSE-BUNDLED
%license LICENSE-FOREIGN
%doc %{name}/doc/_build/html


%changelog
* Tue Jan 09 2024 G3proxy Maintainers <g3proxy-maintainers@devel.machine> - 1.7.33-1
- New upstream release
