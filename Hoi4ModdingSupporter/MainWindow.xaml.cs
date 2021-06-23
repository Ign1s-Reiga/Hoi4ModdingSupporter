﻿using Microsoft.WindowsAPICodePack.Dialogs;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Navigation;
using System.Windows.Shapes;

namespace Hoi4ModdingSupporter
{
    /// <summary>
    /// MainWindow.xaml の相互作用ロジック
    /// </summary>
    public partial class MainWindow : Window
    {
        private string fullPath;
        private string folderName;

        public MainWindow()
        {
            InitializeComponent();
        }

        private void newProject(object sender, RoutedEventArgs e)
        {
            CommonOpenFileDialog dialog = new CommonOpenFileDialog();
            dialog.IsFolderPicker = true;
            dialog.Title = "Select Project Folder...";
            dialog.InitialDirectory = @"C:\Windows\System32";

            if (dialog.ShowDialog() == CommonFileDialogResult.Ok)
            {
                this.fullPath = dialog.FileName;
                string[] folderNames = fullPath.Split(Convert.ToChar(@"\"));
                this.folderName = folderNames.Last();
            }


        }

        private void showAbout(object sender, RoutedEventArgs e)
        {
            string message = "Hoi4 Modding Supporter\nVersion: alpha-1";
            string title = "About";
            MessageBoxButton button = MessageBoxButton.OK;

            MessageBox.Show(message, title, button);
        }

        private void endApplication(object sender, RoutedEventArgs e)
        {
            this.Close();
        }
    }
}
