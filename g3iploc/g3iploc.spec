%if 0%{?rhel} > 7
%undefine _debugsource_packages
%endif

%if 0%{?rhel} == 7
%global debug_package %{nil}
%endif

%define build_profile release-lto

Name:           g3iploc
Version:        0.1.0
Release:        1%{?dist}
Summary:        IP Locate Service for G3 Project

License:        Apache-2.0
URL:            https://github.com/bytedance/g3
Source0:        %{name}-%{version}.tar.xz

%description
IP Locate Service for G3 Project


%prep
%autosetup


%build
G3_PACKAGE_VERSION="%{version}-%{release}"
export G3_PACKAGE_VERSION
cargo build --frozen --offline --profile %{build_profile} --package g3iploc --package g3iploc-db


%install
rm -rf $RPM_BUILD_ROOT
install -m 755 -D target/%{build_profile}/g3iploc %{buildroot}%{_bindir}/g3iploc
install -m 755 -D target/%{build_profile}/g3iploc-db %{buildroot}%{_bindir}/g3iploc-db
install -m 644 -D %{name}/service/g3iploc@.service %{buildroot}/lib/systemd/system/g3iploc@.service


%files
%{_bindir}/g3iploc
%{_bindir}/g3iploc-db
/lib/systemd/system/g3iploc@.service
%license LICENSE
%license LICENSE-BUNDLED
%license LICENSE-FOREIGN


%changelog
* Fri Apr 19 2024 G3iploc Maintainers <g3iploc-maintainers@devel.machine> - 0.1.0-1
- New upstream release
