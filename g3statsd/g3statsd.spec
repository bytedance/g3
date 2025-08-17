
%undefine _debugsource_packages
%define build_profile release-lto

Name:           g3statsd
Version:        0.1.0
Release:        1%{?dist}
Summary:        Keyless server for G3 Project

License:        Apache-2.0
URL:            https://github.com/bytedance/g3
Source0:        %{name}-%{version}.tar.xz

BuildRequires:  gcc, make, pkgconf
BuildRequires:  openssl-devel,

%description
StatsD server for G3 Project


%prep
%autosetup


%build
G3_PACKAGE_VERSION="%{version}-%{release}"
export G3_PACKAGE_VERSION
cargo build --frozen --offline --profile %{build_profile} --package g3statsd --package g3statsd-ctl
sh %{name}/service/generate_systemd.sh


%install
rm -rf $RPM_BUILD_ROOT
install -m 755 -D target/%{build_profile}/g3statsd %{buildroot}%{_bindir}/g3statsd
install -m 755 -D target/%{build_profile}/g3statsd-ctl %{buildroot}%{_bindir}/g3statsd-ctl
install -m 644 -D %{name}/service/g3statsd@.service %{buildroot}/lib/systemd/system/g3statsd@.service


%files
%{_bindir}/g3statsd
%{_bindir}/g3statsd-ctl
/lib/systemd/system/g3statsd@.service
%license LICENSE
%license LICENSE-BUNDLED
%license LICENSE-FOREIGN


%changelog
* Tue May 13 2025 G3statsd Maintainers <g3statsd-maintainers@devel.machine> - 0.1.0-1
- New upstream release
