using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;

namespace Hoi4ModdingSupporter
{
    public class ProjectInfoFile
    {
        public ProjectInfoFile()
        {

        }

        public void createInfoFile(string projectName, string path)
        {
            FileStream stream = File.Create(path + @"\.project");
            string[] lines = { "projectName: " + projectName, "version: Latest", "createTime: " + DateTime.Now.ToString(), "lastModified: none" };

            StreamWriter writer = new StreamWriter(stream);

            foreach (string line in lines)
            {
                writer.WriteLine(line);
            }
            writer.Close();
            writer.Dispose();
            stream.Close();
            stream.Dispose();
        }

        public List<string> readInfoFile(string path)
        {
            StreamReader reader = new StreamReader(path + @"\.project");
            List<string> list = new List<string>();

            string data;
            while ((data = reader.ReadLine()) != null)
                list.Add(data);
            reader.Close();
            reader.Dispose();

            return list;
        }

        public void saveInfoFile(string path)
        {
            try
            {
                StreamWriter writer = new StreamWriter(path + @"\.project");
                int row = File.ReadAllLines(path + @"\.project").Length;

                for (int i = 0; i < row; i++) {
                    if (i == (row - 1))
                    {
                        
                    }
                }
            } 
            catch (IOException e)
            {
                Console.WriteLine(e.Message);
            }
        }
    }
}